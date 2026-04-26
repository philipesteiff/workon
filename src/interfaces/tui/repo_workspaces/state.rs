use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::RepositoryWorkspace;

use super::super::keys::is_plain_character;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::interfaces::tui) struct RepoWorkspaceDialogState {
    open: bool,
    focus: RepoWorkspaceDialogFocus,
    input: String,
    selected: usize,
    dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepoWorkspaceDialogFocus {
    List,
    Add,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::interfaces::tui) enum RepoWorkspaceDialogAction {
    None,
    Add(Vec<PathBuf>),
    Remove(PathBuf),
    Close { refresh: bool },
    MissingInput,
    MissingSelection,
}

impl Default for RepoWorkspaceDialogState {
    fn default() -> Self {
        Self {
            open: false,
            focus: RepoWorkspaceDialogFocus::Add,
            input: String::new(),
            selected: 0,
            dirty: false,
        }
    }
}

impl RepoWorkspaceDialogState {
    pub(in crate::interfaces::tui) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn focus(&self) -> RepoWorkspaceDialogFocus {
        self.focus
    }

    pub(super) fn input(&self) -> &str {
        &self.input
    }

    pub(super) fn selected(&self) -> usize {
        self.selected
    }

    pub(in crate::interfaces::tui) fn open(
        &mut self,
        selected: usize,
        workspaces: &[RepositoryWorkspace],
    ) {
        self.open = true;
        self.input.clear();
        self.dirty = false;
        self.selected = selected.min(workspaces.len().saturating_sub(1));
        self.focus = if workspaces.is_empty() {
            RepoWorkspaceDialogFocus::Add
        } else {
            RepoWorkspaceDialogFocus::List
        };
    }

    pub(super) fn close(&mut self) -> bool {
        let refresh = self.dirty;
        self.open = false;
        self.input.clear();
        self.dirty = false;
        refresh
    }

    pub(in crate::interfaces::tui) fn mark_dirty(&mut self) {
        if self.open {
            self.dirty = true;
        }
    }

    pub(in crate::interfaces::tui) fn sync_workspaces(
        &mut self,
        workspaces: &[RepositoryWorkspace],
    ) {
        self.input.clear();
        self.clamp_selection(workspaces);
        if self.open && workspaces.is_empty() {
            self.focus = RepoWorkspaceDialogFocus::Add;
        }
    }

    pub(in crate::interfaces::tui) fn handle_key(
        &mut self,
        key: KeyEvent,
        workspaces: &[RepositoryWorkspace],
    ) -> RepoWorkspaceDialogAction {
        match key.code {
            KeyCode::Esc => RepoWorkspaceDialogAction::Close {
                refresh: self.close(),
            },
            KeyCode::Tab => {
                self.toggle_focus(workspaces);
                RepoWorkspaceDialogAction::None
            }
            KeyCode::Left => {
                self.focus_list(workspaces);
                RepoWorkspaceDialogAction::None
            }
            KeyCode::Right => {
                self.focus = RepoWorkspaceDialogFocus::Add;
                RepoWorkspaceDialogAction::None
            }
            KeyCode::Down if self.focus == RepoWorkspaceDialogFocus::List => {
                self.move_selection(1, workspaces);
                RepoWorkspaceDialogAction::None
            }
            KeyCode::Up if self.focus == RepoWorkspaceDialogFocus::List => {
                self.move_selection(-1, workspaces);
                RepoWorkspaceDialogAction::None
            }
            KeyCode::Enter => match self.focus {
                RepoWorkspaceDialogFocus::List => self.remove_selected(workspaces),
                RepoWorkspaceDialogFocus::Add => {
                    let paths = parse_paths(&self.input);
                    if paths.is_empty() {
                        return RepoWorkspaceDialogAction::MissingInput;
                    }
                    RepoWorkspaceDialogAction::Add(paths)
                }
            },
            KeyCode::Char(' ')
                if self.focus == RepoWorkspaceDialogFocus::List && is_plain_character(key) =>
            {
                self.remove_selected(workspaces)
            }
            KeyCode::Delete if self.focus == RepoWorkspaceDialogFocus::List => {
                self.remove_selected(workspaces)
            }
            KeyCode::Char('-')
                if self.focus == RepoWorkspaceDialogFocus::List && is_plain_character(key) =>
            {
                self.remove_selected(workspaces)
            }
            KeyCode::Backspace if self.focus == RepoWorkspaceDialogFocus::Add => {
                self.input.pop();
                RepoWorkspaceDialogAction::None
            }
            KeyCode::Char(character)
                if self.focus == RepoWorkspaceDialogFocus::Add && is_plain_character(key) =>
            {
                self.input.push(character);
                RepoWorkspaceDialogAction::None
            }
            _ => RepoWorkspaceDialogAction::None,
        }
    }

    fn toggle_focus(&mut self, workspaces: &[RepositoryWorkspace]) {
        self.focus = match self.focus {
            RepoWorkspaceDialogFocus::List => RepoWorkspaceDialogFocus::Add,
            RepoWorkspaceDialogFocus::Add if !workspaces.is_empty() => {
                RepoWorkspaceDialogFocus::List
            }
            RepoWorkspaceDialogFocus::Add => RepoWorkspaceDialogFocus::Add,
        };
    }

    fn focus_list(&mut self, workspaces: &[RepositoryWorkspace]) {
        if !workspaces.is_empty() {
            self.focus = RepoWorkspaceDialogFocus::List;
        }
    }

    fn move_selection(&mut self, delta: isize, workspaces: &[RepositoryWorkspace]) {
        if workspaces.is_empty() {
            self.selected = 0;
            self.focus = RepoWorkspaceDialogFocus::Add;
            return;
        }

        let count = workspaces.len() as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(count) as usize;
    }

    fn remove_selected(&self, workspaces: &[RepositoryWorkspace]) -> RepoWorkspaceDialogAction {
        match workspaces.get(self.selected) {
            Some(workspace) => RepoWorkspaceDialogAction::Remove(workspace.path.clone()),
            None => RepoWorkspaceDialogAction::MissingSelection,
        }
    }

    fn clamp_selection(&mut self, workspaces: &[RepositoryWorkspace]) {
        if workspaces.is_empty() {
            self.selected = 0;
        } else if self.selected >= workspaces.len() {
            self.selected = workspaces.len() - 1;
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

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{RepoWorkspaceDialogAction, RepoWorkspaceDialogFocus, RepoWorkspaceDialogState};
    use crate::domain::RepositoryWorkspace;

    #[test]
    fn opens_existing_workspaces_with_list_focused() {
        let workspaces = workspaces();
        let mut dialog = RepoWorkspaceDialogState::default();

        dialog.open(0, &workspaces);

        assert!(dialog.is_open());
        assert_eq!(dialog.focus(), RepoWorkspaceDialogFocus::List);
        assert_eq!(
            dialog.handle_key(key(KeyCode::Char(' ')), &workspaces),
            RepoWorkspaceDialogAction::Remove("/tmp/repos".into())
        );
    }

    #[test]
    fn list_focus_ignores_text_so_selection_is_stable() {
        let workspaces = workspaces();
        let mut dialog = RepoWorkspaceDialogState::default();
        dialog.open(0, &workspaces);

        dialog.handle_key(key(KeyCode::Char('/')), &workspaces);
        dialog.handle_key(key(KeyCode::Down), &workspaces);

        assert_eq!(dialog.input(), "");
        assert_eq!(dialog.selected(), 1);
        assert_eq!(dialog.focus(), RepoWorkspaceDialogFocus::List);
    }

    #[test]
    fn add_focus_accepts_paths() {
        let workspaces = workspaces();
        let mut dialog = RepoWorkspaceDialogState::default();
        dialog.open(0, &workspaces);
        dialog.handle_key(key(KeyCode::Tab), &workspaces);

        for character in "/tmp/more,/tmp/client".chars() {
            dialog.handle_key(key(KeyCode::Char(character)), &workspaces);
        }

        assert_eq!(
            dialog.handle_key(key(KeyCode::Enter), &workspaces),
            RepoWorkspaceDialogAction::Add(vec!["/tmp/more".into(), "/tmp/client".into()])
        );
    }

    #[test]
    fn tab_returns_to_list_for_removal() {
        let workspaces = workspaces();
        let mut dialog = RepoWorkspaceDialogState::default();
        dialog.open(0, &workspaces);
        dialog.handle_key(key(KeyCode::Tab), &workspaces);
        dialog.handle_key(key(KeyCode::Char('/')), &workspaces);

        dialog.handle_key(key(KeyCode::Tab), &workspaces);
        dialog.handle_key(key(KeyCode::Down), &workspaces);

        assert_eq!(
            dialog.handle_key(key(KeyCode::Enter), &workspaces),
            RepoWorkspaceDialogAction::Remove("/tmp/client-repos".into())
        );
    }

    fn workspaces() -> Vec<RepositoryWorkspace> {
        vec![
            RepositoryWorkspace {
                path: "/tmp/repos".into(),
            },
            RepositoryWorkspace {
                path: "/tmp/client-repos".into(),
            },
        ]
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
}
