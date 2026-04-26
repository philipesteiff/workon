use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use crate::application::{App, Command, CommandOutput};
use crate::domain::{AttachedRepository, WorkSummary};
use crate::shared::error::{Result, WorkonError};

use super::state::{TraceKind, TuiState};

pub(super) struct AttachedRepositoryIndexJob {
    generation: u64,
    receiver: Receiver<Result<BTreeMap<String, Vec<AttachedRepository>>>>,
}

pub(super) fn spawn_attached_repository_index(
    app: App,
    state: &mut TuiState,
) -> AttachedRepositoryIndexJob {
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

pub(super) fn poll_attached_repository_index_job(
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::mpsc;

    use crate::domain::{AttachedRepository, WorkList, WorkSummary};

    use super::{poll_attached_repository_index_job, AttachedRepositoryIndexJob};
    use crate::interfaces::tui::state::TuiState;

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
}
