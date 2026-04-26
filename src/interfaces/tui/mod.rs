mod animation;
mod components;
mod keymap;
mod keys;
mod repo_jobs;
mod repo_state;
mod repo_ui;
mod state;
#[cfg(test)]
mod state_tests;
mod theme;
mod ui;

use std::collections::BTreeMap;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::application::{App, Command, CommandOutput};
use crate::domain::{AttachedRepository, WorkSummary};
use crate::shared::error::{Result, WorkonError};

use self::animation::{input_poll_timeout, AnimationRuntime, AnimationSnapshot};
use self::repo_jobs::{
    run_repo_batch, RepoBatchDelegate, RepoBatchOutcome, RepoChangeRequest, RepoCommandResult,
    RepoStep, RepoStepReport,
};
use self::repo_state::RepoOperation;
use self::state::{Toast, TraceKind, TuiAction, TuiMode, TuiState};
use self::ui::render_for_animation;

const INLINE_VIEWPORT_HEIGHT: u16 = 28;
const REPO_OPERATION_ANIMATION_INTERVAL: Duration = Duration::from_millis(90);

pub(crate) fn run(app: &App, root: PathBuf) -> Result<Option<CommandOutput>> {
    let CommandOutput::WorkList(work_list) = app.execute(Command::ListWorks)? else {
        unreachable!("list works command returns work list");
    };

    let mut state = TuiState::new(work_list)
        .with_intents(app.available_intents())
        .with_root(root)
        .with_current_directory(std::env::current_dir().ok().as_deref());
    state.push_trace(TraceKind::Run, "list loaded");
    let mut terminal = TerminalSession::enter()?;
    run_loop(app, terminal.terminal_mut(), &mut state)
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        let mut raw_mode = RawModeGuard::enter()?;
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(INLINE_VIEWPORT_HEIGHT),
            },
        )?;
        terminal.hide_cursor()?;
        raw_mode.disarm();
        Ok(Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

struct RawModeGuard {
    active: bool,
}

impl RawModeGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        Ok(Self { active: true })
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.terminal.clear();
        let _ = self.terminal.show_cursor();
        let _ = disable_raw_mode();
    }
}

fn run_loop(
    app: &App,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut TuiState,
) -> Result<Option<CommandOutput>> {
    let mut animation = AnimationRuntime::default();
    let mut last_frame = Instant::now();
    let mut repo_load_job: Option<RepoLoadJob> = None;
    let mut attached_repository_index_job =
        Some(spawn_attached_repository_index(app.clone(), state));

    loop {
        poll_repo_load_job(&mut repo_load_job, state);
        poll_attached_repository_index_job(&mut attached_repository_index_job, state);
        let elapsed = last_frame.elapsed();
        last_frame = Instant::now();
        draw_frame(terminal, state, &mut animation, elapsed)?;

        let Some(key) = read_next_key(
            animation.is_animating()
                || state.is_title_activity_active()
                || repo_load_job.is_some()
                || attached_repository_index_job.is_some(),
        )?
        else {
            state.advance_activity_frame();
            continue;
        };

        let before = AnimationSnapshot::from_state(state);
        match state.handle_key(key) {
            TuiAction::None => {}
            TuiAction::Quit => return Ok(None),
            TuiAction::Switch(slug) => {
                return app.execute(Command::OpenWork { query: slug }).map(Some);
            }
            TuiAction::Archive(slug) => match app.execute(Command::ArchiveWork { query: slug }) {
                Ok(CommandOutput::WorkArchived(work)) => {
                    reload_work_list(app, state)?;
                    attached_repository_index_job =
                        Some(spawn_attached_repository_index(app.clone(), state));
                    state.toast = Some(Toast::info(
                        "Work archived",
                        &work.archive_path.display().to_string(),
                    ));
                    state.push_trace(TraceKind::Sync, format!("archived {}", work.slug));
                    state.close_panel();
                }
                Ok(_) => unreachable!("archive work command returns archive output"),
                Err(error) => {
                    state.toast = Some(Toast::error("Archive failed", &error.to_string()));
                    state.push_trace(TraceKind::Err, format!("archive failed: {error}"));
                    state.close_panel();
                }
            },
            TuiAction::Create { goal, intent_id } => {
                match app.execute(Command::CreateWork { goal, intent_id }) {
                    Ok(CommandOutput::WorkCreated(work)) => {
                        reload_work_list(app, state)?;
                        attached_repository_index_job =
                            Some(spawn_attached_repository_index(app.clone(), state));
                        state.clear_filter_context();
                        state.select_slug(&work.slug);
                        state.toast = Some(Toast::info(
                            "Work created",
                            &work.path.display().to_string(),
                        ));
                        state.push_trace(TraceKind::Sync, format!("created {}", work.slug));
                        state.reset_create_form();
                        state.close_panel();
                    }
                    Ok(_) => unreachable!("create work command returns create output"),
                    Err(error) => {
                        state.toast = Some(Toast::error("Create failed", &error.to_string()));
                        state.push_trace(TraceKind::Err, format!("create failed: {error}"));
                    }
                }
            }
            TuiAction::OpenRepos(slug) => {
                repo_load_job = Some(spawn_repo_load(app.clone(), slug));
            }
            TuiAction::ApplyRepoChanges {
                work_slug,
                add,
                link,
                remove,
                force_remove,
                workspace,
            } => {
                let request = RepoChangeRequest {
                    work_slug,
                    add,
                    link,
                    remove,
                    force_remove,
                    workspace,
                };
                let outcome = apply_repo_changes(app, terminal, state, &mut animation, &request)?;
                refresh_attached_repositories(app, state, &request.work_slug);
                show_repo_batch_outcome(state, outcome);
            }
            TuiAction::AddRepoWorkspaces { work_slug, paths } => {
                draw_frame(terminal, state, &mut animation, Duration::ZERO)?;
                match app.execute(Command::AddRepositoryWorkspaces {
                    paths: paths.clone(),
                }) {
                    Ok(CommandOutput::RepositoryWorkspaces(workspaces)) => {
                        let count = workspaces.workspaces.len();
                        state.update_repo_workspaces(workspaces.workspaces);
                        record_repo_workspace_added(state, &paths, count);
                        state.start_repo_loading("Loading repository sources");
                        draw_frame(terminal, state, &mut animation, Duration::ZERO)?;
                        refresh_repository_candidates(app, state, &work_slug);
                    }
                    Ok(_) => unreachable!("add repository workspace returns repository workspaces"),
                    Err(error) => {
                        state.toast =
                            Some(Toast::error("Repo workspace failed", &error.to_string()));
                        state.push_repo_log(
                            TraceKind::Err,
                            format!("repo workspace failed: {error}"),
                        );
                        state.push_trace(TraceKind::Err, format!("repo workspace failed: {error}"));
                    }
                }
                if state.repo.work_slug != work_slug {
                    state.push_trace(
                        TraceKind::Warn,
                        format!("repo workspace setup returned for stale work {work_slug}"),
                    );
                }
            }
            TuiAction::RemoveRepoWorkspace { work_slug, path } => {
                draw_frame(terminal, state, &mut animation, Duration::ZERO)?;
                match app.execute(Command::RemoveRepositoryWorkspace { path: path.clone() }) {
                    Ok(CommandOutput::RepositoryWorkspaces(workspaces)) => {
                        let count = workspaces.workspaces.len();
                        state.update_repo_workspaces(workspaces.workspaces);
                        record_repo_workspace_removed(state, &path, count);
                        state.start_repo_loading("Loading repository sources");
                        draw_frame(terminal, state, &mut animation, Duration::ZERO)?;
                        refresh_repository_candidates(app, state, &work_slug);
                    }
                    Ok(_) => {
                        unreachable!("remove repository workspace returns repository workspaces")
                    }
                    Err(error) => {
                        state.toast =
                            Some(Toast::error("Repo workspace failed", &error.to_string()));
                        state.push_repo_log(
                            TraceKind::Err,
                            format!("repo workspace failed: {error}"),
                        );
                        state.push_trace(TraceKind::Err, format!("repo workspace failed: {error}"));
                    }
                }
                if state.repo.work_slug != work_slug {
                    state.push_trace(
                        TraceKind::Warn,
                        format!("repo workspace removal returned for stale work {work_slug}"),
                    );
                }
            }
        }
        animation.observe_transition(before, AnimationSnapshot::from_state(state));
        last_frame = Instant::now();
    }
}

fn record_repo_workspace_added(state: &mut TuiState, paths: &[PathBuf], workspace_count: usize) {
    let configured = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    state.push_repo_log(
        TraceKind::Sync,
        format!("repo workspace added {configured}"),
    );
    state.push_trace(
        TraceKind::Sync,
        format!("repo workspaces configured: {workspace_count}"),
    );
}

fn record_repo_workspace_removed(state: &mut TuiState, path: &Path, workspace_count: usize) {
    state.push_repo_log(
        TraceKind::Sync,
        format!("repo workspace removed {}", path.display()),
    );
    state.push_trace(
        TraceKind::Sync,
        format!("repo workspaces configured: {workspace_count}"),
    );
}

fn refresh_repository_candidates(app: &App, state: &mut TuiState, work_slug: &str) {
    match app.execute(Command::ListRepositoryCandidates {
        query: work_slug.to_string(),
    }) {
        Ok(CommandOutput::RepositoryCandidates(candidates)) => {
            let count = candidates.candidates.len();
            state.update_repo_candidates(candidates.candidates);
            state.push_repo_log(TraceKind::Sync, format!("discovered {count} local repos"));
        }
        Ok(_) => unreachable!("list repository candidates command returns candidates"),
        Err(error) => {
            state.push_repo_log(
                TraceKind::Warn,
                format!("local repo discovery unavailable: {error}"),
            );
            state.push_trace(
                TraceKind::Warn,
                format!("local repo discovery unavailable: {error}"),
            );
            state.finish_repo_loading();
        }
    }
}

fn draw_frame(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &TuiState,
    animation: &mut AnimationRuntime,
    elapsed: Duration,
) -> Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        let regions = render_for_animation(frame, state);
        animation.prepare_frame(&regions);
        animation.process_frame(elapsed, frame.buffer_mut(), area);
    })?;
    Ok(())
}

fn read_next_key(animating: bool) -> Result<Option<KeyEvent>> {
    if let Some(timeout) = input_poll_timeout(animating) {
        if !event::poll(timeout)? {
            return Ok(None);
        }
    }

    Ok(match event::read()? {
        Event::Key(key) => Some(key),
        _ => None,
    })
}

fn reload_work_list(app: &App, state: &mut TuiState) -> Result<()> {
    let CommandOutput::WorkList(work_list) = app.execute(Command::ListWorks)? else {
        unreachable!("list works command returns work list");
    };
    state.set_work_list(work_list);
    Ok(())
}

struct AttachedRepositoryIndexJob {
    generation: u64,
    receiver: Receiver<Result<BTreeMap<String, Vec<AttachedRepository>>>>,
}

fn spawn_attached_repository_index(app: App, state: &mut TuiState) -> AttachedRepositoryIndexJob {
    let generation = state.start_repository_index_loading();
    state.push_trace(TraceKind::Run, "repository index loading");
    let works = state.works.clone();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(load_attached_repository_index(&app, &works));
    });

    AttachedRepositoryIndexJob {
        generation,
        receiver,
    }
}

fn poll_attached_repository_index_job(
    job: &mut Option<AttachedRepositoryIndexJob>,
    state: &mut TuiState,
) {
    let Some(active_job) = job else {
        return;
    };

    let result = match active_job.receiver.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => return,
        Err(TryRecvError::Disconnected) => Err(WorkonError::RepositoryContext {
            message: "repository index loader stopped before returning a result".to_string(),
        }),
    };

    if active_job.generation != state.repository_index_generation {
        *job = None;
        return;
    }

    match result {
        Ok(repositories) => {
            let work_count = repositories.len();
            state.set_attached_repositories(repositories);
            state.push_trace(
                TraceKind::Sync,
                format!("repository index loaded for {work_count} works"),
            );
        }
        Err(error) => {
            state.finish_repository_index_loading();
            state.push_trace(
                TraceKind::Warn,
                format!("repository index unavailable: {error}"),
            );
        }
    }

    *job = None;
}

fn load_attached_repository_index(
    app: &App,
    works: &[WorkSummary],
) -> Result<BTreeMap<String, Vec<AttachedRepository>>> {
    let mut repositories = BTreeMap::new();

    for work in works {
        let CommandOutput::WorkRepositories(attached) =
            app.execute(Command::ListWorkRepositories {
                query: work.slug.clone(),
            })?
        else {
            unreachable!("list work repositories command returns work repositories");
        };
        repositories.insert(work.slug.clone(), attached.repositories);
    }

    Ok(repositories)
}

struct RepoContextData {
    work: crate::domain::WorkSummary,
    available: Vec<crate::domain::AvailableRepository>,
    candidates: Vec<crate::domain::RepositoryCandidate>,
    attached: Vec<crate::domain::AttachedRepository>,
    workspaces: Vec<crate::domain::RepositoryWorkspace>,
    catalog_error: Option<String>,
    candidate_error: Option<String>,
}

struct RepoLoadJob {
    work_slug: String,
    receiver: Receiver<Result<RepoContextData>>,
}

fn spawn_repo_load(app: App, work_slug: String) -> RepoLoadJob {
    let (sender, receiver) = mpsc::channel();
    let worker_slug = work_slug.clone();
    thread::spawn(move || {
        let _ = sender.send(load_repo_context(&app, &worker_slug));
    });

    RepoLoadJob {
        work_slug,
        receiver,
    }
}

fn poll_repo_load_job(job: &mut Option<RepoLoadJob>, state: &mut TuiState) {
    let Some(active_job) = job else {
        return;
    };

    let result = match active_job.receiver.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => return,
        Err(TryRecvError::Disconnected) => Err(WorkonError::RepositoryContext {
            message: "repository loader stopped before returning a result".to_string(),
        }),
    };

    if state.mode == TuiMode::Repos && state.repo.work_slug == active_job.work_slug {
        apply_repo_load_result(state, result);
    }
    *job = None;
}

fn apply_repo_load_result(state: &mut TuiState, result: Result<RepoContextData>) {
    match result {
        Ok(repo_context) => {
            let available_count = repo_context.available.len();
            let candidate_count = repo_context.candidates.len();
            let attached_count = repo_context.attached.len();
            let catalog_error = repo_context.catalog_error;
            let candidate_error = repo_context.candidate_error;
            state.enter_repo_context(
                repo_context.work.slug,
                repo_context.work.title,
                repo_context.available,
                repo_context.candidates,
                repo_context.attached,
                repo_context.workspaces,
            );
            state.push_repo_log(
                TraceKind::Sync,
                format!(
                    "loaded {available_count} GitHub repos, {candidate_count} local repos, {attached_count} attached"
                ),
            );
            state.push_trace(TraceKind::Run, "repo context loaded");
            if let Some(error) = catalog_error {
                state.toast = Some(Toast::error(
                    "GitHub catalog unavailable",
                    "Attached repositories can still be removed.",
                ));
                state.push_trace(
                    TraceKind::Warn,
                    format!("GitHub catalog unavailable: {error}"),
                );
                state.set_repo_failed(format!("GitHub catalog unavailable: {error}"));
            }
            if let Some(error) = candidate_error {
                state.push_trace(
                    TraceKind::Warn,
                    format!("local repo discovery unavailable: {error}"),
                );
                state.push_repo_log(
                    TraceKind::Warn,
                    format!("local repo discovery unavailable: {error}"),
                );
            }
        }
        Err(error) => {
            state.toast = Some(Toast::error("Repos failed", &error.to_string()));
            state.push_trace(TraceKind::Err, format!("repos failed: {error}"));
            state.set_repo_failed(error.to_string());
        }
    }
}

fn load_repo_context(app: &App, work_slug: &str) -> Result<RepoContextData> {
    let CommandOutput::WorkRepositories(attached) = app.execute(Command::ListWorkRepositories {
        query: work_slug.to_string(),
    })?
    else {
        unreachable!("list work repositories command returns work repositories");
    };

    let (available, catalog_error) = match app.execute(Command::ListGitHubRepositories) {
        Ok(CommandOutput::RepositoryCatalog(catalog)) => (catalog.repositories, None),
        Ok(_) => unreachable!("list GitHub repositories command returns repository catalog"),
        Err(error) => (Vec::new(), Some(error.to_string())),
    };

    let CommandOutput::RepositoryWorkspaces(workspaces) =
        app.execute(Command::ListRepositoryWorkspaces)?
    else {
        unreachable!("list repository workspaces command returns repository workspaces");
    };

    let (candidates, candidate_error) = if workspaces.workspaces.is_empty() {
        (Vec::new(), None)
    } else {
        match app.execute(Command::ListRepositoryCandidates {
            query: work_slug.to_string(),
        }) {
            Ok(CommandOutput::RepositoryCandidates(candidates)) => (candidates.candidates, None),
            Ok(_) => unreachable!("list repository candidates command returns candidates"),
            Err(error) => (Vec::new(), Some(error.to_string())),
        }
    };

    Ok(RepoContextData {
        work: attached.work,
        available,
        candidates,
        attached: attached.repositories,
        workspaces: workspaces.workspaces,
        catalog_error,
        candidate_error,
    })
}

fn refresh_attached_repositories(app: &App, state: &mut TuiState, work_slug: &str) {
    state.start_repo_step(RepoOperation::Refresh, 1, 1, work_slug);
    match app.execute(Command::ListWorkRepositories {
        query: work_slug.to_string(),
    }) {
        Ok(CommandOutput::WorkRepositories(attached)) => {
            let count = attached.repositories.len();
            state.update_attached_repositories(attached.repositories);
            state.push_repo_log(TraceKind::Sync, format!("refreshed {count} attached repos"));
        }
        Ok(_) => unreachable!("list work repositories command returns work repositories"),
        Err(error) => {
            state.toast = Some(Toast::error("Refresh failed", &error.to_string()));
            state.push_trace(TraceKind::Err, format!("repo refresh failed: {error}"));
            state.set_repo_failed(format!("refresh failed: {error}"));
        }
    }
}

fn apply_repo_changes(
    app: &App,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut TuiState,
    animation: &mut AnimationRuntime,
    request: &RepoChangeRequest,
) -> Result<RepoBatchOutcome> {
    let mut delegate = TuiRepoBatchDelegate {
        app,
        terminal,
        state,
        animation,
    };
    run_repo_batch(request.clone(), &mut delegate)
}

struct TuiRepoBatchDelegate<'a> {
    app: &'a App,
    terminal: &'a mut Terminal<CrosstermBackend<Stdout>>,
    state: &'a mut TuiState,
    animation: &'a mut AnimationRuntime,
}

impl RepoBatchDelegate for TuiRepoBatchDelegate<'_> {
    fn run_step(&mut self, step: &RepoStep) -> Result<RepoCommandResult> {
        self.state
            .start_repo_step(step.operation, step.current, step.total, &step.repository);
        draw_frame(self.terminal, self.state, self.animation, Duration::ZERO)?;
        let receiver = spawn_repo_command(self.app.clone(), step.command.clone());
        wait_for_repo_command(receiver, self.terminal, self.state, self.animation)
    }

    fn record_report(&mut self, report: RepoStepReport) -> Result<()> {
        self.state.push_repo_log(report.kind, report.message);
        draw_frame(self.terminal, self.state, self.animation, Duration::ZERO)
    }
}

fn spawn_repo_command(app: App, command: Command) -> Receiver<Result<CommandOutput>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(app.execute(command));
    });
    receiver
}

fn wait_for_repo_command(
    receiver: Receiver<Result<CommandOutput>>,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut TuiState,
    animation: &mut AnimationRuntime,
) -> Result<RepoCommandResult> {
    let mut cancel_requested = false;
    loop {
        match receiver.recv_timeout(REPO_OPERATION_ANIMATION_INTERVAL) {
            Ok(result) => {
                return Ok(RepoCommandResult {
                    result,
                    cancel_requested,
                })
            }
            Err(RecvTimeoutError::Timeout) => {
                if !cancel_requested && escape_pressed()? {
                    cancel_requested = true;
                    state.push_repo_log(
                        TraceKind::Warn,
                        "cancel requested; finishing current repository operation",
                    );
                    state.toast = Some(Toast::info(
                        "Repository cancel requested",
                        "Current repository operation will finish first.",
                    ));
                }
                state.advance_activity_frame();
                draw_frame(
                    terminal,
                    state,
                    animation,
                    REPO_OPERATION_ANIMATION_INTERVAL,
                )?;
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Ok(RepoCommandResult {
                    result: Err(WorkonError::RepositoryContext {
                        message: "repository operation stopped before returning a result"
                            .to_string(),
                    }),
                    cancel_requested,
                });
            }
        }
    }
}

fn escape_pressed() -> Result<bool> {
    while event::poll(Duration::ZERO)? {
        if matches!(event::read()?, Event::Key(key) if key.code == KeyCode::Esc) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn show_repo_batch_outcome(state: &mut TuiState, outcome: RepoBatchOutcome) {
    if outcome.cancelled {
        state.toast = Some(Toast::info(
            "Repository context cancelled",
            &format!(
                "{} applied, {} failed, {} skipped.",
                outcome.successes,
                outcome.failures.len(),
                outcome.skipped
            ),
        ));
        state.push_trace(
            TraceKind::Warn,
            format!("repo context cancelled: {} skipped", outcome.skipped),
        );
        for failure in outcome.failures {
            state.push_trace(TraceKind::Err, failure);
        }
    } else if outcome.failures.is_empty() {
        state.toast = Some(Toast::info(
            "Repository context updated",
            &format!("{} repository changes applied.", outcome.successes),
        ));
        state.push_trace(
            TraceKind::Sync,
            format!("repo context: {} applied", outcome.successes),
        );
    } else {
        let message = outcome
            .failures
            .first()
            .map(|failure| first_error_line(failure))
            .unwrap_or_else(|| {
                format!(
                    "{} applied, {} failed.",
                    outcome.successes,
                    outcome.failures.len()
                )
            });
        let title = if outcome.successes == 0 {
            "Repository context failed"
        } else {
            "Repository context partially updated"
        };
        state.toast = Some(Toast::error(title, &message));
        for failure in outcome.failures {
            state.push_trace(TraceKind::Err, failure);
        }
    }
}

fn first_error_line(message: &str) -> String {
    message
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(message)
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::mpsc;
    use std::time::Duration;

    use crate::domain::{AttachedRepository, WorkList, WorkSummary};

    use super::animation::{
        input_poll_timeout, AnimationRuntime, AnimationSnapshot, AnimationTarget,
        ANIMATION_FRAME_INTERVAL,
    };
    use super::state::{Toast, TraceKind, TuiMode, TuiState};
    use super::{
        apply_repo_load_result, poll_attached_repository_index_job, record_repo_workspace_added,
        record_repo_workspace_removed, show_repo_batch_outcome, AttachedRepositoryIndexJob,
        RepoBatchOutcome, RepoContextData,
    };

    #[test]
    fn repo_batch_failure_toast_shows_first_failure_reason() {
        let mut state = TuiState::new(work_list());
        show_repo_batch_outcome(
            &mut state,
            RepoBatchOutcome {
                successes: 0,
                failures: vec![
                    "example/private-repo: failed to remove repository link\nrepository context command failed".to_string(),
                ],
                cancelled: false,
                skipped: 0,
            },
        );

        let toast = state.toast.expect("failure toast should be shown");
        assert_eq!(toast.title, "Repository context failed");
        assert_eq!(
            toast.message,
            "example/private-repo: failed to remove repository link"
        );
        assert_eq!(state.trace[0].kind, TraceKind::Err);
    }

    #[test]
    fn repo_load_keeps_attached_repositories_when_github_catalog_fails() {
        let mut state = TuiState::new(work_list());
        let work = work_list().works[0].clone();

        apply_repo_load_result(
            &mut state,
            Ok(RepoContextData {
                work,
                available: Vec::new(),
                candidates: Vec::new(),
                attached: attached_repositories(),
                workspaces: Vec::new(),
                catalog_error: Some("gh auth required".to_string()),
                candidate_error: None,
            }),
        );

        assert_eq!(state.mode, TuiMode::Repos);
        assert_eq!(state.repo.attached.len(), 1);
        assert_eq!(state.repo.attached[0].name_with_owner, "openai/workon");
        assert!(state
            .repo
            .catalog_rows()
            .iter()
            .any(|row| row.name() == "openai/workon"));
        let toast = state.toast.expect("catalog failure should show a toast");
        assert_eq!(toast.title, "GitHub catalog unavailable");
    }

    #[test]
    fn repo_workspace_add_records_logs_without_toast() {
        let mut state = TuiState::new(work_list());

        record_repo_workspace_added(&mut state, &["/tmp/repos".into()], 1);

        assert_eq!(state.toast, None);
        assert!(state
            .repo
            .logs
            .iter()
            .any(|event| event.message == "repo workspace added /tmp/repos"));
        assert!(state
            .trace
            .iter()
            .any(|event| event.message == "repo workspaces configured: 1"));
    }

    #[test]
    fn repo_workspace_remove_records_logs_without_toast() {
        let mut state = TuiState::new(work_list());

        record_repo_workspace_removed(&mut state, std::path::Path::new("/tmp/repos"), 0);

        assert_eq!(state.toast, None);
        assert!(state
            .repo
            .logs
            .iter()
            .any(|event| event.message == "repo workspace removed /tmp/repos"));
        assert!(state
            .trace
            .iter()
            .any(|event| event.message == "repo workspaces configured: 0"));
    }

    #[test]
    fn stale_repository_index_job_does_not_overwrite_newer_attached_repositories() {
        let mut state = TuiState::new(work_list());
        let generation = state.start_repository_index_loading();
        state.repo.work_slug = "billing-retry-audit".to_string();
        state.update_attached_repositories(attached_repositories());

        let (sender, receiver) = mpsc::channel();
        let mut stale = BTreeMap::new();
        stale.insert("billing-retry-audit".to_string(), Vec::new());
        sender
            .send(Ok(stale))
            .expect("test repository index should send");
        let mut job = Some(AttachedRepositoryIndexJob {
            generation,
            receiver,
        });

        poll_attached_repository_index_job(&mut job, &mut state);

        assert!(job.is_none());
        assert_eq!(
            state
                .attached_repositories
                .get("billing-retry-audit")
                .expect("newer attached repos should remain")
                .len(),
            1
        );
    }

    #[test]
    fn repo_batch_cancel_toast_reports_skipped_repositories() {
        let mut state = TuiState::new(work_list());
        show_repo_batch_outcome(
            &mut state,
            RepoBatchOutcome {
                successes: 1,
                failures: Vec::new(),
                cancelled: true,
                skipped: 2,
            },
        );

        let toast = state.toast.expect("cancel toast should be shown");
        assert_eq!(toast.title, "Repository context cancelled");
        assert_eq!(toast.message, "1 applied, 0 failed, 2 skipped.");
        assert_eq!(state.trace[0].kind, TraceKind::Warn);
    }

    #[test]
    fn animation_runtime_enqueues_targets_for_visible_state_transitions() {
        let before = TuiState::new(work_list());
        let mut after = before.clone();
        after.mode = TuiMode::Search;
        after.move_selection(1);
        after.trace_visible = true;
        after.toast = Some(Toast::info("Work created", "/tmp/workon/.workon/work/new"));
        after.works.push(WorkSummary {
            title: "New Work".to_string(),
            slug: "new-work".to_string(),
            goal: "Create a new animated Work.".to_string(),
            intent_id: "investigate".to_string(),
            path: "/tmp/workon/.workon/work/new-work".into(),
        });

        let mut runtime = AnimationRuntime::default();
        runtime.observe_transition(
            AnimationSnapshot::from_state(&before),
            AnimationSnapshot::from_state(&after),
        );

        assert!(!runtime.has_pending(AnimationTarget::Overlay));
        assert!(runtime.has_pending(AnimationTarget::WorkQueue));
        assert!(runtime.has_pending(AnimationTarget::TracePanel));
        assert!(!runtime.has_pending(AnimationTarget::DetailPanel));
        assert!(runtime.has_pending(AnimationTarget::Toast));
        assert!(runtime.has_pending(AnimationTarget::FooterStatus));
    }

    #[test]
    fn animation_runtime_keeps_list_navigation_instant() {
        let before = TuiState::new(work_list());
        let mut after = before.clone();
        after.move_selection(1);

        let mut runtime = AnimationRuntime::default();
        runtime.observe_transition(
            AnimationSnapshot::from_state(&before),
            AnimationSnapshot::from_state(&after),
        );

        assert!(!runtime.has_pending(AnimationTarget::WorkQueue));
    }

    #[test]
    fn animation_runtime_does_not_animate_filter_input() {
        let mut before = TuiState::new(work_list());
        before.mode = TuiMode::Search;
        let mut after = before.clone();
        after.push_filter_char('b');

        let mut runtime = AnimationRuntime::default();
        runtime.observe_transition(
            AnimationSnapshot::from_state(&before),
            AnimationSnapshot::from_state(&after),
        );

        assert!(!runtime.has_pending(AnimationTarget::WorkQueue));
        assert!(!runtime.has_pending(AnimationTarget::Overlay));
    }

    #[test]
    fn animation_runtime_does_not_animate_leader_entry_or_exit() {
        let before = TuiState::new(work_list());
        let mut leader = before.clone();
        leader.mode = TuiMode::Leader;

        let mut runtime = AnimationRuntime::default();
        runtime.observe_transition(
            AnimationSnapshot::from_state(&before),
            AnimationSnapshot::from_state(&leader),
        );

        assert!(!runtime.has_pending(AnimationTarget::Overlay));
        assert!(!runtime.has_pending(AnimationTarget::FooterStatus));

        let mut runtime = AnimationRuntime::default();
        runtime.observe_transition(
            AnimationSnapshot::from_state(&leader),
            AnimationSnapshot::from_state(&before),
        );

        assert!(!runtime.has_pending(AnimationTarget::Overlay));
        assert!(!runtime.has_pending(AnimationTarget::FooterStatus));
    }

    #[test]
    fn animation_poll_timeout_blocks_when_idle_and_ticks_when_active() {
        assert_eq!(input_poll_timeout(false), None);
        assert_eq!(
            input_poll_timeout(true),
            Some(Duration::from_millis(ANIMATION_FRAME_INTERVAL))
        );
    }

    fn work_list() -> WorkList {
        WorkList {
            works: vec![
                WorkSummary {
                    title: "Billing retry audit".to_string(),
                    slug: "billing-retry-audit".to_string(),
                    goal: "Find why billing retry alerts spiked.".to_string(),
                    intent_id: "investigate".to_string(),
                    path: "/tmp/workon/.workon/work/billing-retry-audit".into(),
                },
                WorkSummary {
                    title: "Review cache invalidation PR".to_string(),
                    slug: "review-cache-invalidation-pr".to_string(),
                    goal: "Review cache invalidation changes.".to_string(),
                    intent_id: "review-pr".to_string(),
                    path: "/tmp/workon/.workon/work/review-cache-invalidation-pr".into(),
                },
            ],
        }
    }

    fn attached_repositories() -> Vec<AttachedRepository> {
        vec![AttachedRepository {
            name_with_owner: "openai/workon".to_string(),
            branch: "workon/billing-retry-audit".to_string(),
            path: "/tmp/workon/.workon/work/billing-retry-audit/repos/openai__workon".into(),
            default_branch: "main".to_string(),
            url: "https://github.com/openai/workon".to_string(),
        }]
    }
}
