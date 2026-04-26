use crate::application::{Command, CommandOutput};
use crate::shared::error::Result;
use std::path::PathBuf;

use super::repo_state::RepoOperation;
use super::state::TraceKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepoChangeRequest {
    pub(super) work_slug: String,
    pub(super) add: Vec<String>,
    pub(super) link: Vec<PathBuf>,
    pub(super) remove: Vec<String>,
    pub(super) force_remove: bool,
    pub(super) workspace: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepoStep {
    pub(super) operation: RepoOperation,
    pub(super) current: usize,
    pub(super) total: usize,
    pub(super) repository: String,
    pub(super) command: Command,
    force_remove: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct RepoBatchOutcome {
    pub(super) successes: usize,
    pub(super) failures: Vec<String>,
    pub(super) cancelled: bool,
    pub(super) skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepoStepReport {
    pub(super) kind: TraceKind,
    pub(super) message: String,
}

pub(super) struct RepoCommandResult {
    pub(super) result: Result<CommandOutput>,
    pub(super) cancel_requested: bool,
}

pub(super) trait RepoBatchDelegate {
    fn run_step(&mut self, step: &RepoStep) -> Result<RepoCommandResult>;
    fn record_report(&mut self, report: RepoStepReport) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepoBatch {
    steps: Vec<RepoStep>,
    next: usize,
    outcome: RepoBatchOutcome,
}

pub(super) fn run_repo_batch(
    request: RepoChangeRequest,
    delegate: &mut impl RepoBatchDelegate,
) -> Result<RepoBatchOutcome> {
    let mut batch = RepoBatch::new(request);

    while let Some(step) = batch.next_step() {
        let command_result = delegate.run_step(&step)?;
        let report = batch.record_result(
            &step,
            command_result.result,
            command_result.cancel_requested,
        );
        delegate.record_report(report)?;
        if batch.is_cancelled() {
            return Ok(batch.finish());
        }
    }

    Ok(batch.finish())
}

impl RepoBatch {
    pub(super) fn new(request: RepoChangeRequest) -> Self {
        let total = request.add.len() + request.link.len() + request.remove.len();
        let mut current = 0;
        let mut steps = Vec::with_capacity(total);

        for repository in request.add {
            current += 1;
            steps.push(RepoStep {
                operation: RepoOperation::Add,
                current,
                total,
                repository: repository.clone(),
                command: Command::AddWorkRepositories {
                    query: request.work_slug.clone(),
                    repositories: vec![repository],
                    workspace: request.workspace.clone(),
                },
                force_remove: false,
            });
        }

        for path in request.link {
            current += 1;
            steps.push(RepoStep {
                operation: RepoOperation::Link,
                current,
                total,
                repository: path.display().to_string(),
                command: Command::LinkWorkRepositories {
                    query: request.work_slug.clone(),
                    paths: vec![path],
                },
                force_remove: false,
            });
        }

        for repository in request.remove {
            current += 1;
            steps.push(RepoStep {
                operation: RepoOperation::Remove,
                current,
                total,
                repository: repository.clone(),
                command: Command::RemoveWorkRepositories {
                    query: request.work_slug.clone(),
                    repositories: vec![repository],
                    force: request.force_remove,
                },
                force_remove: request.force_remove,
            });
        }

        Self {
            steps,
            next: 0,
            outcome: RepoBatchOutcome::default(),
        }
    }

    pub(super) fn next_step(&mut self) -> Option<RepoStep> {
        let step = self.steps.get(self.next).cloned()?;
        self.next += 1;
        Some(step)
    }

    pub(super) fn record_result(
        &mut self,
        step: &RepoStep,
        result: Result<CommandOutput>,
        cancel_requested: bool,
    ) -> RepoStepReport {
        let report = match result {
            Ok(output) => {
                let message = step.success_message(&output);
                if step.changed_repositories(&output) > 0 {
                    self.outcome.successes += 1;
                }
                RepoStepReport {
                    kind: TraceKind::Sync,
                    message,
                }
            }
            Err(error) => {
                let failure = format!("{}: {error}", step.repository);
                self.outcome.failures.push(failure.clone());
                RepoStepReport {
                    kind: TraceKind::Err,
                    message: failure,
                }
            }
        };

        if cancel_requested {
            self.outcome.cancelled = true;
            self.outcome.skipped = self.steps.len().saturating_sub(self.next);
        }

        report
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.outcome.cancelled
    }

    pub(super) fn finish(self) -> RepoBatchOutcome {
        self.outcome
    }
}

impl RepoStep {
    fn success_message(&self, output: &CommandOutput) -> String {
        if matches!(self.operation, RepoOperation::Add | RepoOperation::Link)
            && self.changed_repositories(output) == 0
        {
            return format!("already attached {}", self.repository);
        }

        match self.operation {
            RepoOperation::Add => format!("attached {}", self.repository),
            RepoOperation::Link => format!("linked {}", self.repository),
            RepoOperation::Remove if self.force_remove => {
                format!("force removed {}", self.repository)
            }
            RepoOperation::Remove => format!("removed {}", self.repository),
            RepoOperation::Refresh => format!("refreshed {}", self.repository),
        }
    }

    fn changed_repositories(&self, output: &CommandOutput) -> usize {
        match output {
            CommandOutput::WorkRepositoriesAdded(change)
            | CommandOutput::WorkRepositoriesRemoved(change) => change.repositories.len(),
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::shared::error::WorkonError;

    use super::{
        run_repo_batch, RepoBatch, RepoBatchDelegate, RepoChangeRequest, RepoCommandResult,
        RepoStep, RepoStepReport,
    };

    #[test]
    fn batch_orders_adds_before_removes_with_progress_counts() {
        let mut batch = RepoBatch::new(request());

        let first = batch.next_step().expect("first step");
        let second = batch.next_step().expect("second step");
        let third = batch.next_step().expect("third step");

        assert_eq!(first.current, 1);
        assert_eq!(first.total, 3);
        assert_eq!(first.repository, "openai/api");
        assert_eq!(second.repository, "openai/docs");
        assert_eq!(third.repository, "openai/workon");
    }

    #[test]
    fn batch_records_failure_and_cancelled_skip_count_without_terminal() {
        let mut batch = RepoBatch::new(request());
        let first = batch.next_step().expect("first step");
        let second = batch.next_step().expect("second step");

        batch.record_result(
            &first,
            Ok(crate::application::CommandOutput::WorkRepositoriesAdded(
                repository_change(),
            )),
            false,
        );
        batch.record_result(
            &second,
            Err(WorkonError::RepositoryContext {
                message: "failed".to_string(),
            }),
            true,
        );
        let outcome = batch.finish();

        assert_eq!(outcome.successes, 1);
        assert_eq!(outcome.failures, ["openai/docs: failed"]);
        assert!(outcome.cancelled);
        assert_eq!(outcome.skipped, 1);
    }

    #[test]
    fn batch_runner_executes_steps_through_delegate() {
        let mut delegate = FakeDelegate {
            cancel_on_second: true,
            ran: Vec::new(),
            reports: Vec::new(),
        };

        let outcome = run_repo_batch(request(), &mut delegate).expect("batch should run");

        assert_eq!(delegate.ran, ["openai/api", "openai/docs"]);
        assert_eq!(delegate.reports.len(), 2);
        assert!(outcome.cancelled);
        assert_eq!(outcome.skipped, 1);
    }

    #[test]
    fn batch_reports_duplicate_add_as_already_attached_noop() {
        let mut batch = RepoBatch::new(RepoChangeRequest {
            work_slug: "billing".to_string(),
            add: vec!["openai/api".to_string()],
            link: Vec::new(),
            remove: Vec::new(),
            force_remove: false,
            workspace: None,
        });
        let step = batch.next_step().expect("add step");

        let report = batch.record_result(
            &step,
            Ok(crate::application::CommandOutput::WorkRepositoriesAdded(
                crate::domain::RepositoryContextChange {
                    work: work_summary(),
                    repositories: Vec::new(),
                },
            )),
            false,
        );
        let outcome = batch.finish();

        assert_eq!(report.message, "already attached openai/api");
        assert_eq!(outcome.successes, 0);
        assert!(outcome.failures.is_empty());
    }

    #[test]
    fn batch_links_local_paths_between_adds_and_removes() {
        let mut batch = RepoBatch::new(RepoChangeRequest {
            work_slug: "billing".to_string(),
            add: vec!["openai/api".to_string()],
            link: vec!["/tmp/repos/local-tool".into()],
            remove: vec!["openai/workon".to_string()],
            force_remove: false,
            workspace: None,
        });

        let add = batch.next_step().expect("add step");
        let link = batch.next_step().expect("link step");
        let remove = batch.next_step().expect("remove step");

        assert_eq!(add.repository, "openai/api");
        assert_eq!(
            link.operation,
            crate::interfaces::tui::repo_state::RepoOperation::Link
        );
        assert_eq!(link.repository, "/tmp/repos/local-tool");
        assert!(matches!(
            link.command,
            crate::application::Command::LinkWorkRepositories { .. }
        ));
        assert_eq!(remove.repository, "openai/workon");
    }

    fn request() -> RepoChangeRequest {
        RepoChangeRequest {
            work_slug: "billing".to_string(),
            add: vec!["openai/api".to_string(), "openai/docs".to_string()],
            link: Vec::new(),
            remove: vec!["openai/workon".to_string()],
            force_remove: true,
            workspace: None,
        }
    }

    fn work_summary() -> crate::domain::WorkSummary {
        crate::domain::WorkSummary {
            title: "Billing".to_string(),
            slug: "billing".to_string(),
            goal: "Investigate billing".to_string(),
            intent_id: "investigate".to_string(),
            path: "/tmp/workon/billing".into(),
        }
    }

    fn repository_change() -> crate::domain::RepositoryContextChange {
        crate::domain::RepositoryContextChange {
            work: work_summary(),
            repositories: vec![crate::domain::AttachedRepository {
                name_with_owner: "openai/api".to_string(),
                branch: "main".to_string(),
                path: "/tmp/workon/billing/api".into(),
                default_branch: "main".to_string(),
                url: "https://github.com/openai/api".to_string(),
            }],
        }
    }

    struct FakeDelegate {
        cancel_on_second: bool,
        ran: Vec<String>,
        reports: Vec<String>,
    }

    impl RepoBatchDelegate for FakeDelegate {
        fn run_step(&mut self, step: &RepoStep) -> crate::shared::error::Result<RepoCommandResult> {
            self.ran.push(step.repository.clone());
            Ok(RepoCommandResult {
                result: Ok(crate::application::CommandOutput::WorkRepositoriesAdded(
                    repository_change(),
                )),
                cancel_requested: self.cancel_on_second && self.ran.len() == 2,
            })
        }

        fn record_report(&mut self, report: RepoStepReport) -> crate::shared::error::Result<()> {
            self.reports.push(report.message);
            Ok(())
        }
    }
}
