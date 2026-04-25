use crate::app::{Command, CommandOutput};
use crate::error::Result;

use super::repo_state::RepoOperation;
use super::state::TraceKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepoChangeRequest {
    pub(super) work_slug: String,
    pub(super) add: Vec<String>,
    pub(super) remove: Vec<String>,
    pub(super) force_remove: bool,
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
        let total = request.add.len() + request.remove.len();
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
            Ok(_) => {
                self.outcome.successes += 1;
                RepoStepReport {
                    kind: TraceKind::Sync,
                    message: step.success_message(),
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
    fn success_message(&self) -> String {
        match self.operation {
            RepoOperation::Add => format!("attached {}", self.repository),
            RepoOperation::Remove if self.force_remove => {
                format!("force removed {}", self.repository)
            }
            RepoOperation::Remove => format!("removed {}", self.repository),
            RepoOperation::Refresh => format!("refreshed {}", self.repository),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::error::WorkonError;

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
            Ok(crate::app::CommandOutput::Context(context())),
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

    fn request() -> RepoChangeRequest {
        RepoChangeRequest {
            work_slug: "billing".to_string(),
            add: vec!["openai/api".to_string(), "openai/docs".to_string()],
            remove: vec!["openai/workon".to_string()],
            force_remove: true,
        }
    }

    fn context() -> crate::domain::ContextStatus {
        crate::domain::ContextStatus {
            status: "ok".to_string(),
            message: "ok".to_string(),
        }
    }

    struct FakeDelegate {
        cancel_on_second: bool,
        ran: Vec<String>,
        reports: Vec<String>,
    }

    impl RepoBatchDelegate for FakeDelegate {
        fn run_step(&mut self, step: &RepoStep) -> crate::error::Result<RepoCommandResult> {
            self.ran.push(step.repository.clone());
            Ok(RepoCommandResult {
                result: Ok(crate::app::CommandOutput::Context(context())),
                cancel_requested: self.cancel_on_second && self.ran.len() == 2,
            })
        }

        fn record_report(&mut self, report: RepoStepReport) -> crate::error::Result<()> {
            self.reports.push(report.message);
            Ok(())
        }
    }
}
