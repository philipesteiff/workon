use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use crate::application::{App, Command, CommandOutput};
use crate::shared::error::Result;

use super::repo_load::repo_loading_sources_message;
use super::state::{Toast, TraceKind, TuiMode, TuiState};

#[derive(Clone)]
pub(super) enum RepoWorkspaceRequest {
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

pub(super) struct RepoWorkspaceJob {
    work_slug: String,
    request: RepoWorkspaceRequest,
    receiver: Receiver<RepoWorkspaceMessage>,
}

pub(super) fn spawn_repo_workspace_job(
    app: App,
    request: RepoWorkspaceRequest,
) -> RepoWorkspaceJob {
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

pub(super) fn poll_repo_workspace_job(job: &mut Option<RepoWorkspaceJob>, state: &mut TuiState) {
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use crate::domain::{WorkList, WorkSummary};

    use super::{
        poll_repo_workspace_job, record_repo_workspace_added, record_repo_workspace_removed,
        RepoWorkspaceJob, RepoWorkspaceMessage, RepoWorkspaceRequest,
    };
    use crate::interfaces::tui::state::TuiState;

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

    fn work_list() -> WorkList {
        WorkList {
            works: vec![WorkSummary {
                title: "Billing retry audit".to_string(),
                slug: "billing-retry-audit".to_string(),
                goal: "Find why billing retry alerts spiked.".to_string(),
                intent_id: "investigate".to_string(),
                path: "/tmp/workon/.workon/work/billing-retry-audit".into(),
            }],
        }
    }
}
