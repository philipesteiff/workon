mod animation;
mod components;
mod control_panel;
mod keymap;
mod keys;
mod repo_batch;
mod repo_index;
mod repo_jobs;
mod repo_load;
mod repo_rows;
mod repo_state;
mod repo_ui;
mod repo_workspace;
mod state;
#[cfg(test)]
mod state_tests;
mod terminal;
mod theme;
mod ui;

use std::path::PathBuf;
use std::time::Instant;

use crate::application::{App, Command, CommandOutput};
use crate::shared::error::Result;

use self::animation::{AnimationRuntime, AnimationSnapshot};
use self::repo_batch::{
    apply_repo_changes, refresh_attached_repositories, show_repo_batch_outcome,
};
use self::repo_index::poll_attached_repository_index_job;
use self::repo_index::spawn_attached_repository_index;
use self::repo_jobs::RepoChangeRequest;
use self::repo_load::{poll_repo_load_job, spawn_repo_load, RepoLoadJob};
use self::repo_workspace::{
    poll_repo_workspace_job, spawn_repo_workspace_job, RepoWorkspaceJob, RepoWorkspaceRequest,
};
use self::state::{Toast, TraceKind, TuiAction, TuiState};
use self::terminal::{draw_frame, read_next_key, TerminalSession, TuiTerminal};

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

fn run_loop(
    app: &App,
    terminal: &mut TuiTerminal,
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

fn reload_work_list(app: &App, state: &mut TuiState) -> Result<()> {
    let CommandOutput::WorkList(work_list) = app.execute(Command::ListWorks)? else {
        unreachable!("list works command returns work list");
    };
    state.set_work_list(work_list);
    Ok(())
}
