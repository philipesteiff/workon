use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};

use crate::application::{App, Command, CommandOutput};
use crate::shared::error::{Result, WorkonError};

use super::animation::AnimationRuntime;
use super::repo_jobs::{
    run_repo_batch, RepoBatchDelegate, RepoBatchOutcome, RepoChangeRequest, RepoCommandResult,
    RepoStep, RepoStepReport,
};
use super::repo_state::RepoOperation;
use super::state::{Toast, TraceKind, TuiState};
use super::terminal::{draw_frame, TuiTerminal};

const REPO_OPERATION_ANIMATION_INTERVAL: Duration = Duration::from_millis(90);

pub(super) fn refresh_attached_repositories(app: &App, state: &mut TuiState, work_slug: &str) {
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

pub(super) fn apply_repo_changes(
    app: &App,
    terminal: &mut TuiTerminal,
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
    terminal: &'a mut TuiTerminal,
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
    terminal: &mut TuiTerminal,
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

pub(super) fn show_repo_batch_outcome(state: &mut TuiState, outcome: RepoBatchOutcome) {
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
    use crate::domain::{WorkList, WorkSummary};

    use super::show_repo_batch_outcome;
    use crate::interfaces::tui::repo_jobs::RepoBatchOutcome;
    use crate::interfaces::tui::state::{Toast, TraceKind, TuiState};

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
