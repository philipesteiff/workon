use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::domain::{WorkList, WorkSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiState {
    pub(super) works: Vec<WorkSummary>,
    pub(super) selected: usize,
    pub(super) mode: TuiMode,
    pub(super) filter: String,
    pub(super) command: String,
    pub(super) create_goal: String,
    pub(super) create_intent: usize,
    pub(super) intents: Vec<(String, String)>,
    pub(super) root: PathBuf,
    pub(super) toast: Option<Toast>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TuiMode {
    List,
    Search,
    Command,
    Create,
    Archive,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Toast {
    pub(super) title: String,
    pub(super) message: String,
    pub(super) kind: ToastKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToastKind {
    Info,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TuiAction {
    None,
    Quit,
    Switch(String),
    Archive(String),
    Create { goal: String, intent_id: String },
}

impl TuiState {
    pub(super) fn new(work_list: WorkList) -> Self {
        Self {
            works: work_list.works,
            selected: 0,
            mode: TuiMode::List,
            filter: String::new(),
            command: String::new(),
            create_goal: String::new(),
            create_intent: 0,
            intents: Vec::new(),
            root: PathBuf::new(),
            toast: None,
        }
    }

    pub(super) fn with_intents(mut self, intents: Vec<(String, String)>) -> Self {
        self.intents = intents;
        self.create_intent = 0;
        self
    }

    pub(super) fn with_root(mut self, root: PathBuf) -> Self {
        self.root = root;
        self
    }

    pub(super) fn with_active_work_path(mut self, active_work_path: Option<&Path>) -> Self {
        let Some(active_work_path) = active_work_path else {
            return self;
        };

        if let Some(index) = self
            .works
            .iter()
            .position(|work| work.path == active_work_path)
        {
            self.selected = index;
        }

        self
    }

    pub(super) fn set_work_list(&mut self, work_list: WorkList) {
        self.works = work_list.works;
        self.clamp_selection();
    }

    pub(super) fn filtered_works(&self) -> Vec<&WorkSummary> {
        self.filtered_indices()
            .into_iter()
            .filter_map(|index| self.works.get(index))
            .collect()
    }

    pub(super) fn selected_work(&self) -> Option<&WorkSummary> {
        let indices = self.filtered_indices();
        indices
            .get(self.selected)
            .and_then(|index| self.works.get(*index))
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        let count = self.filtered_indices().len();
        if count == 0 {
            self.selected = 0;
            return;
        }

        let count = count as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(count) as usize;
    }

    pub(super) fn push_filter_char(&mut self, character: char) {
        self.filter.push(character);
        self.selected = 0;
    }

    pub(super) fn select_slug(&mut self, slug: &str) {
        let Some(index) = self
            .filtered_works()
            .iter()
            .position(|work| work.slug == slug)
        else {
            self.clamp_selection();
            return;
        };
        self.selected = index;
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> TuiAction {
        if key.kind != KeyEventKind::Press {
            return TuiAction::None;
        }

        match self.mode {
            TuiMode::List => self.handle_list_key(key),
            TuiMode::Search => self.handle_search_key(key),
            TuiMode::Command => self.handle_command_key(key),
            TuiMode::Create => self.handle_create_key(key),
            TuiMode::Archive => self.handle_archive_key(key),
            TuiMode::Help => self.handle_help_key(key),
        }
    }

    pub(super) fn close_panel(&mut self) {
        self.mode = TuiMode::List;
        self.command.clear();
    }

    pub(super) fn reset_create_form(&mut self) {
        self.create_goal.clear();
        self.create_intent = 0;
    }

    pub(super) fn filtered_indices(&self) -> Vec<usize> {
        let query = self.filter.trim().to_ascii_lowercase();
        self.works
            .iter()
            .enumerate()
            .filter_map(|(index, work)| {
                if query.is_empty() || work_matches(work, &query) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Char('q') => TuiAction::Quit,
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_selection(1);
                TuiAction::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_selection(-1);
                TuiAction::None
            }
            KeyCode::Enter => self
                .selected_work()
                .map(|work| TuiAction::Switch(work.slug.clone()))
                .unwrap_or(TuiAction::None),
            KeyCode::Char('/') => {
                self.mode = TuiMode::Search;
                TuiAction::None
            }
            KeyCode::Char(':') => {
                self.command.clear();
                self.mode = TuiMode::Command;
                TuiAction::None
            }
            KeyCode::Char('n') => {
                self.mode = TuiMode::Create;
                TuiAction::None
            }
            KeyCode::Char('a') => {
                if self.selected_work().is_some() {
                    self.mode = TuiMode::Archive;
                }
                TuiAction::None
            }
            KeyCode::Char('?') => {
                self.mode = TuiMode::Help;
                TuiAction::None
            }
            KeyCode::Char(character) if is_plain_character(key) => {
                self.mode = TuiMode::Search;
                self.push_filter_char(character);
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.mode = TuiMode::List;
                TuiAction::None
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.selected = 0;
                TuiAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(1);
                TuiAction::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(-1);
                TuiAction::None
            }
            KeyCode::Char(character) if is_plain_character(key) => {
                self.push_filter_char(character);
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc => {
                self.close_panel();
                TuiAction::None
            }
            KeyCode::Enter => self.run_command(),
            KeyCode::Backspace => {
                self.command.pop();
                TuiAction::None
            }
            KeyCode::Char(character) if is_plain_character(key) => {
                self.command.push(character);
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn handle_create_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc => {
                self.close_panel();
                TuiAction::None
            }
            KeyCode::Tab | KeyCode::Down => {
                self.next_intent();
                TuiAction::None
            }
            KeyCode::Up => {
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
            KeyCode::Esc | KeyCode::Char('n') => {
                self.close_panel();
                TuiAction::None
            }
            KeyCode::Enter | KeyCode::Char('y') => self
                .selected_work()
                .map(|work| TuiAction::Archive(work.slug.clone()))
                .unwrap_or(TuiAction::None),
            _ => TuiAction::None,
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> TuiAction {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                self.close_panel();
                TuiAction::None
            }
            _ => TuiAction::None,
        }
    }

    fn run_command(&mut self) -> TuiAction {
        let command = self.command.trim().to_ascii_lowercase();
        self.command.clear();

        match command.as_str() {
            "" | "list" => {
                self.close_panel();
                TuiAction::None
            }
            "create" | "new" => {
                self.mode = TuiMode::Create;
                TuiAction::None
            }
            "switch" | "open" => {
                self.close_panel();
                self.selected_work()
                    .map(|work| TuiAction::Switch(work.slug.clone()))
                    .unwrap_or(TuiAction::None)
            }
            "archive" => {
                if self.selected_work().is_some() {
                    self.mode = TuiMode::Archive;
                } else {
                    self.close_panel();
                }
                TuiAction::None
            }
            _ => {
                self.toast = Some(Toast::error("Unknown command", &command));
                self.close_panel();
                TuiAction::None
            }
        }
    }

    fn create_action(&mut self) -> TuiAction {
        let goal = self.create_goal.trim().to_string();
        if goal.is_empty() {
            self.toast = Some(Toast::error(
                "Goal required",
                "Type a Work goal before creating.",
            ));
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

    fn next_intent(&mut self) {
        if !self.intents.is_empty() {
            self.create_intent = (self.create_intent + 1) % self.intents.len();
        }
    }

    fn previous_intent(&mut self) {
        if !self.intents.is_empty() {
            self.create_intent = (self.create_intent + self.intents.len() - 1) % self.intents.len();
        }
    }

    fn clamp_selection(&mut self) {
        let count = self.filtered_indices().len();
        if count == 0 {
            self.selected = 0;
        } else if self.selected >= count {
            self.selected = count - 1;
        }
    }
}

impl Toast {
    pub(super) fn info(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            kind: ToastKind::Info,
        }
    }

    pub(super) fn error(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            kind: ToastKind::Error,
        }
    }
}

fn work_matches(work: &WorkSummary, query: &str) -> bool {
    work.title.to_ascii_lowercase().contains(query)
        || work.slug.to_ascii_lowercase().contains(query)
        || work.intent_id.to_ascii_lowercase().contains(query)
        || work.goal.to_ascii_lowercase().contains(query)
}

fn is_plain_character(key: KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::TuiState;
    use crate::domain::{WorkList, WorkSummary};

    #[test]
    fn filters_work_by_title_slug_intent_and_goal() {
        let mut state = TuiState::new(work_list());

        state.push_filter_char('r');
        state.push_filter_char('e');
        state.push_filter_char('v');

        let filtered = state.filtered_works();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].slug, "review-cache-invalidation-pr");
    }

    #[test]
    fn selection_wraps_through_filtered_work() {
        let mut state = TuiState::new(work_list());

        state.move_selection(-1);
        assert_eq!(
            state.selected_work().map(|work| work.slug.as_str()),
            Some("context-command-sketch")
        );

        state.move_selection(1);
        assert_eq!(
            state.selected_work().map(|work| work.slug.as_str()),
            Some("billing-retry-audit")
        );
    }

    #[test]
    fn selects_active_work_path_when_opening_list() {
        let state = TuiState::new(work_list()).with_active_work_path(Some(std::path::Path::new(
            "/tmp/workon/.workon/work/review-cache-invalidation-pr",
        )));

        assert_eq!(
            state.selected_work().map(|work| work.slug.as_str()),
            Some("review-cache-invalidation-pr")
        );
    }

    fn work_list() -> WorkList {
        WorkList {
            works: vec![
                WorkSummary {
                    title: "Billing retry audit".to_string(),
                    slug: "billing-retry-audit".to_string(),
                    goal: "Find why billing retry alerts spiked after the queue rollout."
                        .to_string(),
                    intent_id: "investigate".to_string(),
                    path: PathBuf::from("/tmp/workon/.workon/work/billing-retry-audit"),
                },
                WorkSummary {
                    title: "Review cache invalidation PR".to_string(),
                    slug: "review-cache-invalidation-pr".to_string(),
                    goal: "Review the cache invalidation PR for regressions.".to_string(),
                    intent_id: "review-pr".to_string(),
                    path: PathBuf::from("/tmp/workon/.workon/work/review-cache-invalidation-pr"),
                },
                WorkSummary {
                    title: "Context command sketch".to_string(),
                    slug: "context-command-sketch".to_string(),
                    goal: "Shape the first context surface.".to_string(),
                    intent_id: "brainstorm".to_string(),
                    path: PathBuf::from("/tmp/workon/.workon/work/context-command-sketch"),
                },
            ],
        }
    }
}
