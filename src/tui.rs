mod components;
mod state;
mod theme;
mod ui;

use std::io::{self, Stdout};
use std::path::PathBuf;

use crossterm::event::{self, Event};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::app::{App, Command, CommandOutput};
use crate::error::Result;
use crate::shell_integration::current_work_path;

use self::state::{Toast, TraceKind, TuiAction, TuiState};
use self::ui::render;

const INLINE_VIEWPORT_HEIGHT: u16 = 22;

pub(crate) fn run(app: &App, root: PathBuf) -> Result<Option<CommandOutput>> {
    let CommandOutput::WorkList(work_list) = app.execute(Command::ListWorks)? else {
        unreachable!("list works command returns work list");
    };

    let mut state = TuiState::new(work_list)
        .with_intents(app.available_intents())
        .with_root(root)
        .with_active_work_path(current_work_path().as_deref());
    state.push_trace(TraceKind::Run, "list loaded");
    let mut terminal = TerminalSession::enter()?;
    run_loop(app, terminal.terminal_mut(), &mut state)
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(INLINE_VIEWPORT_HEIGHT),
            },
        )?;
        terminal.hide_cursor()?;
        Ok(Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = self.terminal.show_cursor();
    }
}

fn run_loop(
    app: &App,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut TuiState,
) -> Result<Option<CommandOutput>> {
    loop {
        terminal.draw(|frame| render(frame, state))?;

        let Event::Key(key) = event::read()? else {
            continue;
        };

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
    }
}

fn reload_work_list(app: &App, state: &mut TuiState) -> Result<()> {
    let CommandOutput::WorkList(work_list) = app.execute(Command::ListWorks)? else {
        unreachable!("list works command returns work list");
    };
    state.set_work_list(work_list);
    Ok(())
}
