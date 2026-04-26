use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use super::{RepoPane, RepoPickerState};
use crate::interfaces::tui::keys::is_plain_character;
use crate::interfaces::tui::state::Toast;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::interfaces::tui) enum RepoPickerAction {
    None,
    Back,
    Notify(Toast),
    AddWorkspaces {
        work_slug: String,
        paths: Vec<PathBuf>,
    },
    RemoveWorkspace {
        work_slug: String,
        path: PathBuf,
    },
    Apply {
        work_slug: String,
        add: Vec<String>,
        link: Vec<PathBuf>,
        remove: Vec<String>,
        force_remove: bool,
        workspace: Option<PathBuf>,
    },
}

impl RepoPickerState {
    pub(in crate::interfaces::tui) fn handle_key(&mut self, key: KeyEvent) -> RepoPickerAction {
        match key.code {
            KeyCode::Esc => {
                self.clear_pending_changes();
                self.filter.clear();
                RepoPickerAction::Back
            }
            KeyCode::Tab | KeyCode::Right => {
                self.next_focus();
                RepoPickerAction::None
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.previous_focus();
                RepoPickerAction::None
            }
            KeyCode::Down => {
                self.move_selection(1);
                RepoPickerAction::None
            }
            KeyCode::Up => {
                self.move_selection(-1);
                RepoPickerAction::None
            }
            KeyCode::Backspace if self.focus == RepoPane::AddPath => {
                self.workspace_input.pop();
                RepoPickerAction::None
            }
            KeyCode::Backspace if !self.filter.is_empty() => {
                self.filter.pop();
                self.selected_catalog = 0;
                RepoPickerAction::None
            }
            KeyCode::Char(character)
                if self.focus == RepoPane::AddPath && is_plain_character(key) =>
            {
                self.workspace_input.push(character);
                RepoPickerAction::None
            }
            KeyCode::Char('+') if is_plain_character(key) => {
                self.focus = RepoPane::AddPath;
                RepoPickerAction::None
            }
            KeyCode::Char(' ') if is_plain_character(key) => self.handle_space(),
            KeyCode::Char('!') if self.focus == RepoPane::Catalog && is_plain_character(key) => {
                self.toggle_force_remove()
            }
            KeyCode::Enter => self.handle_enter(),
            KeyCode::Char(character) if is_plain_character(key) => self.handle_character(character),
            _ => RepoPickerAction::None,
        }
    }

    fn handle_space(&mut self) -> RepoPickerAction {
        match self.focus {
            RepoPane::Catalog => {
                self.toggle_focused_repository();
                RepoPickerAction::None
            }
            RepoPane::AddPath => {
                self.workspace_input.push(' ');
                RepoPickerAction::None
            }
            RepoPane::ConfiguredPaths => self.remove_workspace_action(),
        }
    }

    fn handle_enter(&mut self) -> RepoPickerAction {
        match self.focus {
            RepoPane::Catalog => self.apply_action(),
            RepoPane::AddPath => self.add_workspace_action(),
            RepoPane::ConfiguredPaths => self.remove_workspace_action(),
        }
    }

    fn handle_character(&mut self, character: char) -> RepoPickerAction {
        match self.focus {
            RepoPane::Catalog => {
                self.filter.push(character);
                self.selected_catalog = 0;
            }
            RepoPane::AddPath => self.workspace_input.push(character),
            RepoPane::ConfiguredPaths => {}
        }
        RepoPickerAction::None
    }

    fn apply_action(&mut self) -> RepoPickerAction {
        if self.pending_add.is_empty()
            && self.pending_link.is_empty()
            && self.pending_remove.is_empty()
        {
            return RepoPickerAction::None;
        }
        if !self.pending_add.is_empty() && self.workspaces.is_empty() {
            return RepoPickerAction::Notify(Toast::error(
                "No repo workspace",
                "Type a repo workspace path, then press enter.",
            ));
        }

        RepoPickerAction::Apply {
            work_slug: self.work_slug.clone(),
            add: self.pending_add.iter().cloned().collect(),
            link: self.pending_link.iter().cloned().collect(),
            remove: self.pending_remove.iter().cloned().collect(),
            force_remove: self.force_remove && !self.pending_remove.is_empty(),
            workspace: if self.pending_add.is_empty() {
                None
            } else {
                self.selected_workspace_path()
            },
        }
    }

    fn add_workspace_action(&mut self) -> RepoPickerAction {
        let paths = parse_paths(&self.workspace_input);
        if paths.is_empty() {
            return RepoPickerAction::None;
        }
        self.workspace_input.clear();
        self.start_loading("Adding repository workspace");
        RepoPickerAction::AddWorkspaces {
            work_slug: self.work_slug.clone(),
            paths,
        }
    }

    fn remove_workspace_action(&mut self) -> RepoPickerAction {
        match self.workspaces.get(self.selected_workspace) {
            Some(workspace) => {
                let path = workspace.path.clone();
                self.start_loading("Removing repository workspace");
                RepoPickerAction::RemoveWorkspace {
                    work_slug: self.work_slug.clone(),
                    path,
                }
            }
            None => RepoPickerAction::None,
        }
    }
}

fn parse_paths(input: &str) -> Vec<PathBuf> {
    input
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}
