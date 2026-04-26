use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::keys::{is_plain_character, is_shortcut_character};
use super::repo_state::RepoPickerAction;
use super::state::{Toast, TraceKind, TuiAction, TuiMode, TuiState};

impl TuiState {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> TuiAction {
        if key.kind != KeyEventKind::Press {
            return TuiAction::None;
        }

        self.toast = None;

        if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.toggle_trace();
            self.push_trace(
                TraceKind::Run,
                if self.trace_visible {
                    "trace panel shown"
                } else {
                    "trace panel hidden"
                },
            );
            return TuiAction::None;
        }

        match self.mode {
            TuiMode::List => self.handle_list_key(key),
            TuiMode::Search => self.handle_search_key(key),
            TuiMode::Leader => self.handle_leader_key(key),
            TuiMode::Create => self.handle_create_key(key),
            TuiMode::Archive => self.handle_archive_key(key),
            TuiMode::Repos => self.handle_repos_key(key),
            TuiMode::Help => self.handle_help_key(key),
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc => TuiAction::Quit,
            KeyCode::Down => {
                self.move_selection(1);
                TuiAction::None
            }
            KeyCode::Up => {
                self.move_selection(-1);
                TuiAction::None
            }
            KeyCode::Enter => self
                .selected_work()
                .map(|work| TuiAction::Switch(work.slug.clone()))
                .unwrap_or(TuiAction::None),
            KeyCode::Char('/') if is_plain_character(key) => {
                self.mode = TuiMode::Leader;
                self.push_trace(TraceKind::Run, "command leader ready");
                TuiAction::None
            }
            KeyCode::Char(character) if is_plain_character(key) => {
                self.enter_filter_mode();
                self.push_filter_char(character);
                self.push_trace(TraceKind::Signal, filter_trace_message(&self.filter));
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc if self.filter.is_empty() => TuiAction::Quit,
            KeyCode::Esc => {
                self.close_filter_mode();
                TuiAction::None
            }
            KeyCode::Enter => self
                .selected_work()
                .map(|work| TuiAction::Switch(work.slug.clone()))
                .unwrap_or(TuiAction::None),
            KeyCode::Backspace if !self.filter.is_empty() => {
                self.filter.pop();
                self.selected = 0;
                self.push_trace(TraceKind::Signal, filter_trace_message(&self.filter));
                TuiAction::None
            }
            KeyCode::Down => {
                self.move_selection(1);
                TuiAction::None
            }
            KeyCode::Up => {
                self.move_selection(-1);
                TuiAction::None
            }
            KeyCode::Char('/') if is_plain_character(key) => {
                self.mode = TuiMode::Leader;
                self.push_trace(TraceKind::Run, "command leader ready");
                TuiAction::None
            }
            KeyCode::Char(character) if is_plain_character(key) => {
                self.push_filter_char(character);
                self.push_trace(TraceKind::Signal, filter_trace_message(&self.filter));
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_leader_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc => {
                self.restore_queue_mode();
                TuiAction::None
            }
            KeyCode::Char('n') if is_plain_character(key) => {
                self.mode = TuiMode::Create;
                self.push_trace(TraceKind::Run, "work init ready");
                TuiAction::None
            }
            KeyCode::Char('a') if is_plain_character(key) => {
                if self.selected_work().is_some() {
                    self.mode = TuiMode::Archive;
                    self.push_trace(TraceKind::Warn, "archive confirmation armed");
                } else {
                    self.restore_queue_mode();
                }
                TuiAction::None
            }
            KeyCode::Char('r') if is_plain_character(key) => {
                let Some((slug, title)) = self
                    .selected_work()
                    .map(|work| (work.slug.clone(), work.title.clone()))
                else {
                    return TuiAction::None;
                };
                self.enter_repo_loading(slug.clone(), title);
                TuiAction::OpenRepos(slug)
            }
            KeyCode::Char('?') if is_plain_character(key) => {
                self.mode = TuiMode::Help;
                TuiAction::None
            }
            KeyCode::Char('q') if is_plain_character(key) => TuiAction::Quit,
            KeyCode::Char('/') if is_plain_character(key) => {
                self.enter_filter_mode();
                self.push_filter_char('/');
                self.push_trace(TraceKind::Signal, filter_trace_message(&self.filter));
                TuiAction::None
            }
            _ => {
                self.restore_queue_mode();
                TuiAction::None
            }
        }
    }

    fn handle_create_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc => {
                self.close_panel();
                TuiAction::None
            }
            KeyCode::Tab | KeyCode::Down | KeyCode::Right => {
                self.next_intent();
                TuiAction::None
            }
            KeyCode::BackTab | KeyCode::Up | KeyCode::Left => {
                self.previous_intent();
                TuiAction::None
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => self.create_action(),
            KeyCode::Enter => self.create_action(),
            KeyCode::Backspace => {
                self.create_goal.pop();
                TuiAction::None
            }
            KeyCode::Char(character) if is_plain_character(key) => {
                self.create_goal.push(character);
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_archive_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc => {
                self.close_panel();
                TuiAction::None
            }
            KeyCode::Char('n') if is_shortcut_character(key) => {
                self.close_panel();
                TuiAction::None
            }
            KeyCode::Enter => self
                .selected_work()
                .map(|work| TuiAction::Archive(work.slug.clone()))
                .unwrap_or(TuiAction::None),
            KeyCode::Char('y') if is_shortcut_character(key) => self
                .selected_work()
                .map(|work| TuiAction::Archive(work.slug.clone()))
                .unwrap_or(TuiAction::None),
            _ => TuiAction::None,
        }
    }

    fn handle_repos_key(&mut self, key: KeyEvent) -> TuiAction {
        match self.repo.handle_key(key) {
            RepoPickerAction::None => TuiAction::None,
            RepoPickerAction::Back => {
                self.restore_queue_mode();
                TuiAction::None
            }
            RepoPickerAction::Notify(toast) => {
                self.toast = Some(toast);
                TuiAction::None
            }
            RepoPickerAction::AddWorkspaces { work_slug, paths } => {
                TuiAction::AddRepoWorkspaces { work_slug, paths }
            }
            RepoPickerAction::RemoveWorkspace { work_slug, path } => {
                TuiAction::RemoveRepoWorkspace { work_slug, path }
            }
            RepoPickerAction::CloseWorkspaceDialog { work_slug, refresh } => {
                if refresh {
                    TuiAction::RefreshRepoIndex { work_slug }
                } else if self.repo.requires_workspace_setup() {
                    self.restore_queue_mode();
                    TuiAction::None
                } else {
                    TuiAction::None
                }
            }
            RepoPickerAction::Apply {
                work_slug,
                add,
                link,
                remove,
                force_remove,
                workspace,
            } => TuiAction::ApplyRepoChanges {
                work_slug,
                add,
                link,
                remove,
                force_remove,
                workspace,
            },
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc => {
                self.close_panel();
                TuiAction::None
            }
            KeyCode::Char('?') if is_plain_character(key) => {
                self.close_panel();
                TuiAction::None
            }
            KeyCode::Char('q') if is_shortcut_character(key) => {
                self.close_panel();
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn create_action(&mut self) -> TuiAction {
        let goal = self.create_goal.trim().to_string();
        if goal.is_empty() {
            self.toast = Some(Toast::error(
                "Goal required",
                "Type a Work goal before creating.",
            ));
            self.push_trace(TraceKind::Err, "work init missing goal");
            return TuiAction::None;
        }

        TuiAction::Create {
            goal,
            intent_id: self.selected_intent_id().to_string(),
        }
    }

    fn selected_intent_id(&self) -> &str {
        self.intents
            .get(self.create_intent)
            .map(|(id, _)| id.as_str())
            .unwrap_or("investigate")
    }
}

fn filter_trace_message(filter: &str) -> String {
    if filter.is_empty() {
        "filter cleared".to_string()
    } else {
        format!("filter {filter}")
    }
}
