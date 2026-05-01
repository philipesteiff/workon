use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::application::{App, Command, CommandOutput};
use crate::shared::error::Result;

use super::state::{Toast, TraceKind, TuiMode, TuiState};

pub(super) struct RepoAttachedData {
    work: crate::domain::WorkSummary,
    attached: Vec<crate::domain::AttachedRepository>,
}

enum RepoLoadMessage {
    Attached(Result<RepoAttachedData>),
    Workspaces(Result<Vec<crate::domain::RepositoryWorkspace>>),
    CandidatePaths(Result<Vec<crate::domain::RepositoryCandidatePath>>),
    CandidateInspection {
        path: PathBuf,
        result: Result<crate::domain::RepositoryCandidateInspection>,
    },
    CandidateInspectionsFinished,
    Catalog(Result<Vec<crate::domain::AvailableRepository>>),
    Finished,
}

const CANDIDATE_INSPECTION_WORKERS: usize = 8;

pub(super) struct RepoLoadJob {
    work_slug: String,
    receiver: Receiver<RepoLoadMessage>,
}

pub(super) fn spawn_repo_load(app: App, work_slug: String) -> RepoLoadJob {
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

pub(super) fn poll_repo_load_job(job: &mut Option<RepoLoadJob>, state: &mut TuiState) {
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
        RepoLoadMessage::CandidatePaths(Ok(paths)) => {
            let count = paths.len();
            state.update_repo_candidate_paths_from_load(paths);
            state.push_repo_log(TraceKind::Sync, format!("queued {count} local repo paths"));
        }
        RepoLoadMessage::CandidatePaths(Err(error)) => {
            state.push_repo_log(
                TraceKind::Warn,
                format!("local repo path discovery unavailable: {error}"),
            );
            state.push_trace(
                TraceKind::Warn,
                format!("local repo path discovery unavailable: {error}"),
            );
        }
        RepoLoadMessage::CandidateInspection { path, result } => match result {
            Ok(inspection) => state.update_repo_candidate_inspection_from_load(inspection),
            Err(error) => {
                state.mark_repo_candidate_inspection_failed(path.clone());
                state.push_repo_log(
                    TraceKind::Warn,
                    format!(
                        "local repo inspection failed for {}: {error}",
                        path.display()
                    ),
                );
            }
        },
        RepoLoadMessage::CandidateInspectionsFinished => state
            .repo
            .update_loading_message(repo_loading_sources_message(&["GitHub"])),
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
            state.push_repo_log(
                TraceKind::Warn,
                format!("GitHub catalog unavailable: {error}"),
            );
        }
        RepoLoadMessage::Finished => state.finish_repo_loading(),
    }
}

pub(super) fn repo_loading_sources_message(sources: &[&str]) -> String {
    format!("Loading: {}", sources.join(", "))
}

fn load_repo_context_sources(app: &App, work_slug: &str, sender: Sender<RepoLoadMessage>) {
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

    let candidate_workers = if workspaces.is_empty() {
        Vec::new()
    } else {
        match load_repo_candidate_paths(app, work_slug) {
            Ok(paths) => {
                if sender
                    .send(RepoLoadMessage::CandidatePaths(Ok(paths.clone())))
                    .is_err()
                {
                    return;
                }
                spawn_candidate_path_inspections(app, paths, &sender)
            }
            Err(error) => {
                let _ = sender.send(RepoLoadMessage::CandidatePaths(Err(error)));
                Vec::new()
            }
        }
    };

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

    let _ = sender.send(RepoLoadMessage::Catalog(load_repo_catalog(app)));
    for worker in candidate_workers {
        let _ = worker.join();
    }
    if !workspaces.is_empty() {
        let _ = sender.send(RepoLoadMessage::CandidateInspectionsFinished);
    }
    let _ = sender.send(RepoLoadMessage::Finished);
}

fn spawn_candidate_path_inspections(
    app: &App,
    paths: Vec<crate::domain::RepositoryCandidatePath>,
    sender: &Sender<RepoLoadMessage>,
) -> Vec<thread::JoinHandle<()>> {
    let queue = Arc::new(Mutex::new(
        paths
            .into_iter()
            .map(|candidate_path| candidate_path.path)
            .collect::<VecDeque<_>>(),
    ));
    let worker_count = CANDIDATE_INSPECTION_WORKERS.min(queue.lock().map(|q| q.len()).unwrap_or(0));
    let mut workers = Vec::new();

    for _ in 0..worker_count {
        let app = app.clone();
        let queue = Arc::clone(&queue);
        let sender = sender.clone();
        workers.push(thread::spawn(move || loop {
            let path = match queue.lock() {
                Ok(mut queue) => queue.pop_front(),
                Err(_) => None,
            };
            let Some(path) = path else {
                break;
            };
            let result = inspect_repo_candidate(&app, &path);
            if sender
                .send(RepoLoadMessage::CandidateInspection { path, result })
                .is_err()
            {
                break;
            }
        }));
    }

    workers
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

fn load_repo_candidate_paths(
    app: &App,
    work_slug: &str,
) -> Result<Vec<crate::domain::RepositoryCandidatePath>> {
    let CommandOutput::RepositoryCandidatePaths(paths) =
        app.execute(Command::ListRepositoryCandidatePaths {
            query: work_slug.to_string(),
        })?
    else {
        unreachable!("list repository candidate paths command returns candidate paths");
    };
    Ok(paths.paths)
}

fn inspect_repo_candidate(
    app: &App,
    path: &std::path::Path,
) -> Result<crate::domain::RepositoryCandidateInspection> {
    let CommandOutput::RepositoryCandidateInspection(inspection) =
        app.execute(Command::InspectRepositoryCandidate {
            path: path.to_path_buf(),
            refresh: true,
        })?
    else {
        unreachable!("inspect repository candidate command returns candidate inspection");
    };
    Ok(inspection)
}

fn load_repo_catalog(app: &App) -> Result<Vec<crate::domain::AvailableRepository>> {
    let CommandOutput::RepositoryCatalog(catalog) = app.execute(Command::ListGitHubRepositories)?
    else {
        unreachable!("list GitHub repositories command returns repository catalog");
    };
    Ok(catalog.repositories)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use crate::application::{App, Command};
    use crate::domain::{AttachedRepository, WorkList, WorkSummary};

    use super::{
        apply_repo_load_message, load_repo_context_sources, poll_repo_load_job, RepoAttachedData,
        RepoLoadJob, RepoLoadMessage,
    };
    use crate::interfaces::tui::repo_state::RepoStatus;
    use crate::interfaces::tui::state::{TuiMode, TuiState};

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
        let mut job = Some(RepoLoadJob {
            work_slug: work.slug,
            receiver,
        });

        poll_repo_load_job(&mut job, &mut state);

        assert!(job.is_some());
        assert_eq!(state.repo.attached.len(), 1);
        assert_eq!(state.repo.attached[0].name_with_owner, "openai/workon");
    }

    #[test]
    fn repo_load_sends_workspaces_before_attached_and_optional_sources() {
        let root = temp_root("repo_load_fast_first_source");
        let app = App::new(root.path.clone());
        app.execute(Command::CreateWork {
            goal: "Repository speed check".to_string(),
            intent_id: "blank".to_string(),
        })
        .expect("work should be created");

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            load_repo_context_sources(&app, "repository-speed-check", sender);
        });

        let first = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("loader should send a fast first source");

        assert!(matches!(first, RepoLoadMessage::Workspaces(Ok(_))));
    }

    #[test]
    fn repo_load_sends_candidate_paths_before_attached_repositories() {
        let root = temp_root("repo_load_candidate_paths_before_attached");
        let app = App::new(root.path.clone());
        app.execute(Command::CreateWork {
            goal: "Repository speed check".to_string(),
            intent_id: "blank".to_string(),
        })
        .expect("work should be created");
        let workspace = root.path.join("repos");
        std::fs::create_dir_all(workspace.join("local-tool/.git"))
            .expect("candidate folder should be created");
        app.execute(Command::AddRepositoryWorkspaces {
            paths: vec![workspace],
        })
        .expect("workspace should be configured");

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            load_repo_context_sources(&app, "repository-speed-check", sender);
        });

        let first = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("loader should send workspaces first");
        let second = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("loader should send candidate paths second");

        assert!(matches!(first, RepoLoadMessage::Workspaces(Ok(_))));
        assert!(matches!(second, RepoLoadMessage::CandidatePaths(Ok(_))));
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
        assert!(state.is_title_activity_active());

        let candidate_path = std::path::PathBuf::from("/tmp/repos/local-tool");
        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::CandidatePaths(Ok(vec![crate::domain::RepositoryCandidatePath {
                path: candidate_path.clone(),
                cached: None,
            }])),
        );
        assert_eq!(state.repo.pending_candidate_paths.len(), 1);

        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::CandidateInspection {
                path: candidate_path.clone(),
                result: Ok(crate::domain::RepositoryCandidateInspection {
                    path: candidate_path,
                    candidate: Some(crate::domain::RepositoryCandidate {
                        name_with_owner: "openai/local-tool".to_string(),
                        branch: "main".to_string(),
                        path: "/tmp/repos/local-tool".into(),
                        url: "https://github.com/openai/local-tool".to_string(),
                    }),
                    cached: false,
                }),
            },
        );
        assert_eq!(state.repo.candidates.len(), 1);
        apply_repo_load_message(&mut state, RepoLoadMessage::CandidateInspectionsFinished);
        assert_eq!(repo_loading_message(&state), "Loading: GitHub");
        assert!(state.is_title_activity_active());
    }

    #[test]
    fn repo_load_keeps_failed_candidate_path_as_warning_row() {
        let mut state = TuiState::new(work_list());
        let work = work_list().works[0].clone();
        let path = std::path::PathBuf::from("/tmp/repos/broken");
        state.enter_repo_loading(work.slug.clone(), work.title.clone());
        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::CandidatePaths(Ok(vec![crate::domain::RepositoryCandidatePath {
                path: path.clone(),
                cached: None,
            }])),
        );

        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::CandidateInspection {
                path: path.clone(),
                result: Err(crate::shared::error::WorkonError::RepositoryContext {
                    message: "git failed".to_string(),
                }),
            },
        );

        assert!(state.repo.failed_candidate_paths.contains(&path));
        assert!(state.repo.catalog_rows().iter().any(|row| matches!(
            row,
            crate::interfaces::tui::repo_state::RepoCatalogRow::PendingLocal {
                path: row_path,
                failed: true
            } if row_path == &path
        )));
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
        assert!(state.is_title_activity_active());
    }

    #[test]
    fn repo_load_keeps_ready_state_when_github_catalog_fails_late() {
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
        apply_repo_load_message(
            &mut state,
            RepoLoadMessage::Catalog(Err(crate::shared::error::WorkonError::RepositoryContext {
                message: "gh timed out".to_string(),
            })),
        );
        assert!(state.is_title_activity_active());

        apply_repo_load_message(&mut state, RepoLoadMessage::Finished);

        assert!(matches!(state.repo.status, RepoStatus::Ready));
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
        let RepoStatus::Loading { message } = &state.repo.status else {
            panic!("expected repo loading status");
        };
        message
    }

    struct TempRoot {
        path: std::path::PathBuf,
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn temp_root(name: &str) -> TempRoot {
        let mut path = std::env::temp_dir();
        path.push(format!("workon-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temp root should be created");
        TempRoot { path }
    }
}
