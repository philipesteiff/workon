use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::shared::error::Result;

use super::animation::{input_poll_timeout, AnimationRuntime};
use super::state::TuiState;
use super::ui::render_for_animation;

const INLINE_VIEWPORT_HEIGHT: u16 = 28;

pub(super) type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub(super) struct TerminalSession {
    terminal: TuiTerminal,
}

impl TerminalSession {
    pub(super) fn enter() -> Result<Self> {
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

    pub(super) fn terminal_mut(&mut self) -> &mut TuiTerminal {
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

pub(super) fn draw_frame(
    terminal: &mut TuiTerminal,
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

pub(super) fn read_next_key(animating: bool) -> Result<Option<KeyEvent>> {
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
