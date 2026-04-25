mod animation;
mod components;
mod state;
mod theme;
mod ui;

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Instant;

use crossterm::event::{self, Event, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::app::{App, Command, CommandOutput};
use crate::error::Result;

use self::animation::{input_poll_timeout, AnimationRuntime, AnimationSnapshot};
use self::state::{Toast, TraceKind, TuiAction, TuiState};
use self::ui::render_for_animation;

const INLINE_VIEWPORT_HEIGHT: u16 = 28;

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

    loop {
        let elapsed = last_frame.elapsed();
        last_frame = Instant::now();
        terminal.draw(|frame| {
            let area = frame.area();
            let regions = render_for_animation(frame, state);
            animation.prepare_frame(&regions);
            animation.process_frame(elapsed, frame.buffer_mut(), area);
        })?;

        let Some(key) = read_next_key(animation.is_animating())? else {
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
                        state.filter.clear();
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
        }
        animation.observe_transition(before, AnimationSnapshot::from_state(state));
        last_frame = Instant::now();
    }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::{WorkList, WorkSummary};

    use super::animation::{
        input_poll_timeout, AnimationRuntime, AnimationSnapshot, AnimationTarget,
        ANIMATION_FRAME_INTERVAL,
    };
    use super::state::{Toast, TuiMode, TuiState};

    #[test]
    fn animation_runtime_enqueues_targets_for_visible_state_transitions() {
        let before = TuiState::new(work_list());
        let mut after = before.clone();
        after.mode = TuiMode::Search;
        after.move_selection(1);
        after.trace_visible = true;
        after.detail_visible = true;
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
        assert!(runtime.has_pending(AnimationTarget::DetailPanel));
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
}
