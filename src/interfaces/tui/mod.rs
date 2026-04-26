mod animation;
mod components;
mod control_panel;
mod keymap;
mod keys;
mod repo_jobs;
mod repo_rows;
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
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
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
    let mut repo_workspace_job: Option<RepoWorkspaceJob> = None;
    let mut attached_repository_index_job =
        Some(spawn_attached_repository_index(app.clone(), state));

    loop {
        poll_repo_load_job(&mut repo_load_job, state);
        poll_repo_workspace_job(&mut repo_workspace_job, state);
        poll_attached_repository_index_job(&mut attached_repository_index_job, state);
        let elapsed = last_frame.elapsed();
        last_frame = Instant::now();
        draw_frame(terminal, state, &mut animation, elapsed)?;

        let Some(key) = read_next_key(
            animation.is_animating()
                || state.is_title_activity_active()
                || repo_load_job.is_some()
                || repo_workspace_job.is_some()
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
                repo_load_job = None;
                repo_workspace_job = None;
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
                repo_load_job = None;
                repo_workspace_job = Some(spawn_repo_workspace_job(
                    app.clone(),
                    RepoWorkspaceRequest::Add { work_slug, paths },
                ));
            }
            TuiAction::RemoveRepoWorkspace { work_slug, path } => {
                repo_load_job = None;
                repo_workspace_job = Some(spawn_repo_workspace_job(
                    app.clone(),
                    RepoWorkspaceRequest::Remove { work_slug, path },
                ));
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

#[derive(Clone)]
enum RepoWorkspaceRequest {
    Add {
        work_slug: String,
        paths: Vec<PathBuf>,
    },
    Remove {
        work_slug: String,
        path: PathBuf,
    },
}

impl RepoWorkspaceRequest {
    fn work_slug(&self) -> &str {
        match self {
            Self::Add { work_slug, .. } | Self::Remove { work_slug, .. } => work_slug,
        }
    }
}

enum RepoWorkspaceMessage {
    Workspaces(Result<Vec<crate::domain::RepositoryWorkspace>>),
    Candidates(Result<Vec<crate::domain::RepositoryCandidate>>),
    Finished,
}

struct RepoWorkspaceJob {
    work_slug: String,
    request: RepoWorkspaceRequest,
    receiver: Receiver<RepoWorkspaceMessage>,
}

fn spawn_repo_workspace_job(app: App, request: RepoWorkspaceRequest) -> RepoWorkspaceJob {
    let work_slug = request.work_slug().to_string();
    let worker_request = request.clone();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        run_repo_workspace_job(&app, worker_request, sender);
    });

    RepoWorkspaceJob {
        work_slug,
        request,
        receiver,
    }
}

fn poll_repo_workspace_job(job: &mut Option<RepoWorkspaceJob>, state: &mut TuiState) {
    let Some(active_job) = job.as_ref() else {
        return;
    };

    if state.mode != TuiMode::Repos || state.repo.work_slug != active_job.work_slug {
        *job = None;
        return;
    }

    loop {
        let message = match job
            .as_ref()
            .expect("repo workspace job should exist")
            .receiver
            .try_recv()
        {
            Ok(message) => message,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                state.toast = Some(Toast::error(
                    "Repo workspace failed",
                    "repository workspace job stopped before returning a result",
                ));
                state.push_trace(
                    TraceKind::Err,
                    "repo workspace job stopped before returning a result",
                );
                state.set_repo_failed("repository workspace job stopped before returning a result");
                *job = None;
                return;
            }
        };

        if matches!(message, RepoWorkspaceMessage::Finished) {
            state.finish_repo_loading();
            *job = None;
            return;
        }

        let request = job
            .as_ref()
            .expect("repo workspace job should exist")
            .request
            .clone();
        apply_repo_workspace_message(state, &request, message);
    }
}

fn apply_repo_workspace_message(
    state: &mut TuiState,
    request: &RepoWorkspaceRequest,
    message: RepoWorkspaceMessage,
) {
    match message {
        RepoWorkspaceMessage::Workspaces(Ok(workspaces)) => {
            let count = workspaces.len();
            state.update_repo_workspaces_from_load(workspaces);
            match request {
                RepoWorkspaceRequest::Add { paths, .. } => {
                    record_repo_workspace_added(state, paths, count);
                }
                RepoWorkspaceRequest::Remove { path, .. } => {
                    record_repo_workspace_removed(state, path, count);
                }
            }
            state.start_repo_loading(repo_loading_sources_message(&["local"]));
        }
        RepoWorkspaceMessage::Workspaces(Err(error)) => {
            state.toast = Some(Toast::error("Repo workspace failed", &error.to_string()));
            state.push_repo_log(TraceKind::Err, format!("repo workspace failed: {error}"));
            state.push_trace(TraceKind::Err, format!("repo workspace failed: {error}"));
            state.set_repo_failed(error.to_string());
        }
        RepoWorkspaceMessage::Candidates(Ok(candidates)) => {
            let count = candidates.len();
            state.update_repo_candidates_from_load(candidates);
            state.push_repo_log(TraceKind::Sync, format!("discovered {count} local repos"));
        }
        RepoWorkspaceMessage::Candidates(Err(error)) => {
            state.push_repo_log(
                TraceKind::Warn,
                format!("local repo discovery unavailable: {error}"),
            );
            state.push_trace(
                TraceKind::Warn,
                format!("local repo discovery unavailable: {error}"),
            );
        }
        RepoWorkspaceMessage::Finished => {}
    }
}

fn run_repo_workspace_job(
    app: &App,
    request: RepoWorkspaceRequest,
    sender: Sender<RepoWorkspaceMessage>,
) {
    let workspaces = match apply_repo_workspace_request(app, &request) {
        Ok(workspaces) => workspaces,
        Err(error) => {
            let _ = sender.send(RepoWorkspaceMessage::Workspaces(Err(error)));
            let _ = sender.send(RepoWorkspaceMessage::Finished);
            return;
        }
    };

    if sender
        .send(RepoWorkspaceMessage::Workspaces(Ok(workspaces)))
        .is_err()
    {
        return;
    }

    let _ = sender.send(RepoWorkspaceMessage::Candidates(load_repo_candidates(
        app,
        request.work_slug(),
    )));
    let _ = sender.send(RepoWorkspaceMessage::Finished);
}

fn apply_repo_workspace_request(
    app: &App,
    request: &RepoWorkspaceRequest,
) -> Result<Vec<crate::domain::RepositoryWorkspace>> {
    let output = match request {
        RepoWorkspaceRequest::Add { paths, .. } => {
            app.execute(Command::AddRepositoryWorkspaces {
                paths: paths.clone(),
            })?
        }
        RepoWorkspaceRequest::Remove { path, .. } => {
            app.execute(Command::RemoveRepositoryWorkspace { path: path.clone() })?
        }
    };
    let CommandOutput::RepositoryWorkspaces(workspaces) = output else {
        unreachable!("repository workspace commands return repository workspaces");
    };
    Ok(workspaces.workspaces)
}

struct RepoAttachedData {
    work: crate::domain::WorkSummary,
    attached: Vec<crate::domain::AttachedRepository>,
}

enum RepoLoadMessage {
    Attached(Result<RepoAttachedData>),
    Workspaces(Result<Vec<crate::domain::RepositoryWorkspace>>),
    Candidates(Result<Vec<crate::domain::RepositoryCandidate>>),
    Catalog(Result<Vec<crate::domain::AvailableRepository>>),
    Finished,
}

struct RepoLoadJob {
    work_slug: String,
    receiver: Receiver<RepoLoadMessage>,
}

fn spawn_repo_load(app: App, work_slug: String) -> RepoLoadJob {
    let (sender, receiver) = mpsc::channel();
    let worker_slug = work_slug.clone();
    thread::spawn(move || {
        load_repo_context_sources(&app, &worker_slug, sender);
    });

    RepoLoadJob {
        work_slug,
        receiver,
    }
}

fn poll_repo_load_job(job: &mut Option<RepoLoadJob>, state: &mut TuiState) {
    let Some(active_job) = job.as_ref() else {
        return;
    };

    if state.mode != TuiMode::Repos || state.repo.work_slug != active_job.work_slug {
        *job = None;
        return;
    }

    loop {
        let message = match job
            .as_ref()
            .expect("repo job should exist")
            .receiver
            .try_recv()
        {
            Ok(message) => message,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                state.toast = Some(Toast::error(
                    "Repos failed",
                    "repository loader stopped before returning a result",
                ));
                state.push_trace(
                    TraceKind::Err,
                    "repository loader stopped before returning a result",
                );
                state.set_repo_failed("repository loader stopped before returning a result");
                *job = None;
                return;
            }
        };

        if matches!(message, RepoLoadMessage::Finished) {
            state.finish_repo_loading();
            state.push_trace(TraceKind::Run, "repo context loaded");
            *job = None;
            return;
        }

        apply_repo_load_message(state, message);
    }
}

fn apply_repo_load_message(state: &mut TuiState, message: RepoLoadMessage) {
    match message {
        RepoLoadMessage::Attached(Ok(attached)) => {
            let count = attached.attached.len();
            state.repo.work_slug = attached.work.slug;
            state.repo.work_title = attached.work.title;
            state.update_repo_attached_from_load(attached.attached);
            state
                .repo
                .update_loading_message(repo_loading_sources_message(&[
                    "workspaces",
                    "local",
                    "GitHub",
                ]));
            state.push_repo_log(TraceKind::Sync, format!("loaded {count} attached repos"));
        }
        RepoLoadMessage::Attached(Err(error)) => {
            state.toast = Some(Toast::error("Repos failed", &error.to_string()));
            state.push_trace(TraceKind::Err, format!("repos failed: {error}"));
            state.set_repo_failed(error.to_string());
        }
        RepoLoadMessage::Workspaces(Ok(workspaces)) => {
            let count = workspaces.len();
            let loading_sources = if workspaces.is_empty() {
                repo_loading_sources_message(&["GitHub"])
            } else {
                repo_loading_sources_message(&["local", "GitHub"])
            };
            state.update_repo_workspaces_from_load(workspaces);
            state.repo.update_loading_message(loading_sources);
            state.push_repo_log(TraceKind::Sync, format!("loaded {count} repo workspaces"));
        }
        RepoLoadMessage::Workspaces(Err(error)) => {
            state
                .repo
                .update_loading_message(repo_loading_sources_message(&["GitHub"]));
            state.push_repo_log(
                TraceKind::Warn,
                format!("repo workspaces unavailable: {error}"),
            );
            state.push_trace(
                TraceKind::Warn,
                format!("repo workspaces unavailable: {error}"),
            );
        }
        RepoLoadMessage::Candidates(Ok(candidates)) => {
            let count = candidates.len();
            state.update_repo_candidates_from_load(candidates);
            state
                .repo
                .update_loading_message(repo_loading_sources_message(&["GitHub"]));
            state.push_repo_log(TraceKind::Sync, format!("discovered {count} local repos"));
        }
        RepoLoadMessage::Candidates(Err(error)) => {
            state
                .repo
                .update_loading_message(repo_loading_sources_message(&["GitHub"]));
            state.push_repo_log(
                TraceKind::Warn,
                format!("local repo discovery unavailable: {error}"),
            );
            state.push_trace(
                TraceKind::Warn,
                format!("local repo discovery unavailable: {error}"),
            );
        }
        RepoLoadMessage::Catalog(Ok(available)) => {
            let count = available.len();
            state.update_repo_available_from_load(available);
            state.push_repo_log(TraceKind::Sync, format!("loaded {count} GitHub repos"));
        }
        RepoLoadMessage::Catalog(Err(error)) => {
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
        RepoLoadMessage::Finished => {}
    }
}

fn repo_loading_sources_message(sources: &[&str]) -> String {
    format!("Loading: {}", sources.join(", "))
}

fn load_repo_context_sources(app: &App, work_slug: &str, sender: Sender<RepoLoadMessage>) {
    let attached = match load_repo_attached(app, work_slug) {
        Ok(attached) => attached,
        Err(error) => {
            let _ = sender.send(RepoLoadMessage::Attached(Err(error)));
            let _ = sender.send(RepoLoadMessage::Finished);
            return;
        }
    };
    if sender
        .send(RepoLoadMessage::Attached(Ok(RepoAttachedData {
            work: attached.work,
            attached: attached.repositories,
        })))
        .is_err()
    {
        return;
    }

    let workspaces = match load_repo_workspaces(app) {
        Ok(workspaces) => {
            if sender
                .send(RepoLoadMessage::Workspaces(Ok(workspaces.clone())))
                .is_err()
            {
                return;
            }
            workspaces
        }
        Err(error) => {
            let _ = sender.send(RepoLoadMessage::Workspaces(Err(error)));
            Vec::new()
        }
    };

    if !workspaces.is_empty() {
        let _ = sender.send(RepoLoadMessage::Candidates(load_repo_candidates(
            app, work_slug,
        )));
    }

    let _ = sender.send(RepoLoadMessage::Catalog(load_repo_catalog(app)));
    let _ = sender.send(RepoLoadMessage::Finished);
}

fn load_repo_attached(app: &App, work_slug: &str) -> Result<crate::domain::WorkRepositoryList> {
    let CommandOutput::WorkRepositories(attached) = app.execute(Command::ListWorkRepositories {
        query: work_slug.to_string(),
    })?
    else {
        unreachable!("list work repositories command returns work repositories");
    };
    Ok(attached)
}

fn load_repo_workspaces(app: &App) -> Result<Vec<crate::domain::RepositoryWorkspace>> {
    let CommandOutput::RepositoryWorkspaces(workspaces) =
        app.execute(Command::ListRepositoryWorkspaces)?
    else {
        unreachable!("list repository workspaces command returns repository workspaces");
    };
    Ok(workspaces.workspaces)
}

fn load_repo_candidates(
    app: &App,
    work_slug: &str,
) -> Result<Vec<crate::domain::RepositoryCandidate>> {
    let CommandOutput::RepositoryCandidates(candidates) =
        app.execute(Command::ListRepositoryCandidates {
            query: work_slug.to_string(),
        })?
    else {
        unreachable!("list repository candidates command returns candidates");
    };
    Ok(candidates.candidates)
}

fn load_repo_catalog(app: &App) -> Result<Vec<crate::domain::AvailableRepository>> {
    let CommandOutput::RepositoryCatalog(catalog) = app.execute(Command::ListGitHubRepositories)?
    else {
        unreachable!("list GitHub repositories command returns repository catalog");
    };
    Ok(catalog.repositories)
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
        state.toast = None;
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
        apply_repo_load_message, poll_attached_repository_index_job, poll_repo_load_job,
        poll_repo_workspace_job, record_repo_workspace_added, record_repo_workspace_removed,
        show_repo_batch_outcome, AttachedRepositoryIndexJob, RepoAttachedData, RepoBatchOutcome,
        RepoLoadMessage, RepoWorkspaceJob, RepoWorkspaceMessage, RepoWorkspaceRequest,
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
        state.enter_repo_loading(work.slug.clone(), work.title.clone());

        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::Attached(Ok(RepoAttachedData {
                work,
                attached: attached_repositories(),
            })),
        );
        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::Catalog(Err(crate::shared::error::WorkonError::RepositoryContext {
                message: "gh auth required".to_string(),
            })),
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
    fn repo_load_applies_partial_attached_result_before_finished() {
        let mut state = TuiState::new(work_list());
        let work = work_list().works[0].clone();
        state.enter_repo_loading(work.slug.clone(), work.title.clone());
        let (sender, receiver) = mpsc::channel();
        sender
            .send(RepoLoadMessage::Attached(Ok(RepoAttachedData {
                work: work.clone(),
                attached: attached_repositories(),
            })))
            .expect("partial repo load message should send");
        let mut job = Some(super::RepoLoadJob {
            work_slug: work.slug,
            receiver,
        });

        poll_repo_load_job(&mut job, &mut state);

        assert!(job.is_some());
        assert_eq!(state.repo.attached.len(), 1);
        assert_eq!(state.repo.attached[0].name_with_owner, "openai/workon");
    }

    #[test]
    fn repo_load_title_removes_sources_as_they_finish() {
        let mut state = TuiState::new(work_list());
        let work = work_list().works[0].clone();
        state.enter_repo_loading(work.slug.clone(), work.title.clone());

        assert!(repo_loading_message(&state).contains("attached, workspaces, local, GitHub"));

        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::Attached(Ok(RepoAttachedData {
                work,
                attached: attached_repositories(),
            })),
        );
        assert!(repo_loading_message(&state).contains("workspaces, local, GitHub"));
        assert!(!repo_loading_message(&state).contains("attached"));

        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::Workspaces(Ok(vec![crate::domain::RepositoryWorkspace {
                path: "/tmp/repos".into(),
            }])),
        );
        assert!(repo_loading_message(&state).contains("local, GitHub"));
        assert!(!repo_loading_message(&state).contains("workspaces"));

        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::Candidates(Ok(vec![crate::domain::RepositoryCandidate {
                name_with_owner: "openai/local-tool".to_string(),
                branch: "main".to_string(),
                path: "/tmp/repos/local-tool".into(),
                url: "https://github.com/openai/local-tool".to_string(),
            }])),
        );
        assert_eq!(repo_loading_message(&state), "Loading: GitHub");
    }

    #[test]
    fn repo_load_title_skips_local_when_no_workspaces_exist() {
        let mut state = TuiState::new(work_list());
        let work = work_list().works[0].clone();
        state.enter_repo_loading(work.slug.clone(), work.title.clone());

        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::Attached(Ok(RepoAttachedData {
                work,
                attached: attached_repositories(),
            })),
        );
        apply_repo_load_message(&mut state, RepoLoadMessage::Workspaces(Ok(Vec::new())));

        assert_eq!(repo_loading_message(&state), "Loading: GitHub");
    }

    #[test]
    fn repo_workspace_job_applies_paths_before_candidate_refresh_finishes() {
        let mut state = TuiState::new(work_list());
        let work = work_list().works[0].clone();
        state.enter_repo_loading(work.slug.clone(), work.title.clone());
        let path = std::path::PathBuf::from("/tmp/repos");
        let (sender, receiver) = mpsc::channel();
        sender
            .send(RepoWorkspaceMessage::Workspaces(Ok(vec![
                crate::domain::RepositoryWorkspace { path: path.clone() },
            ])))
            .expect("partial repo workspace message should send");
        let mut job = Some(RepoWorkspaceJob {
            work_slug: work.slug.clone(),
            request: RepoWorkspaceRequest::Add {
                work_slug: work.slug,
                paths: vec![path.clone()],
            },
            receiver,
        });

        poll_repo_workspace_job(&mut job, &mut state);

        assert!(job.is_some());
        assert_eq!(state.repo.workspaces.len(), 1);
        assert_eq!(state.repo.workspaces[0].path, path);
        assert!(state
            .repo
            .logs
            .iter()
            .any(|event| event.message == "repo workspace added /tmp/repos"));
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
    fn successful_repo_batch_records_trace_without_toast() {
        let mut state = TuiState::new(work_list());
        state.toast = Some(Toast::info("Previous event", "Keep repositories compact."));

        show_repo_batch_outcome(
            &mut state,
            RepoBatchOutcome {
                successes: 2,
                failures: Vec::new(),
                cancelled: false,
                skipped: 0,
            },
        );

        assert_eq!(state.toast, None);
        assert_eq!(state.trace[0].kind, TraceKind::Sync);
        assert_eq!(state.trace[0].message, "repo context: 2 applied");
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

    fn repo_loading_message(state: &TuiState) -> &str {
        let super::repo_state::RepoStatus::Loading { message } = &state.repo.status else {
            panic!("expected repo loading status");
        };
        message
    }
}
