use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::domain::{WorkList, WorkSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiState {
    pub(super) works: Vec<WorkSummary>,
    pub(super) selected: usize,
    pub(super) mode: TuiMode,
    pub(super) filter: String,
    pub(super) create_goal: String,
    pub(super) create_intent: usize,
    pub(super) intents: Vec<(String, String)>,
    pub(super) root: PathBuf,
    pub(super) active_work_path: Option<PathBuf>,
    pub(super) trace: Vec<TraceEvent>,
    pub(super) trace_visible: bool,
    pub(super) detail_visible: bool,
    pub(super) toast: Option<Toast>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TuiMode {
    List,
    Search,
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
pub(super) struct TraceEvent {
    pub(super) kind: TraceKind,
    pub(super) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TraceKind {
    Run,
    Sync,
    Signal,
    Warn,
    Err,
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
            create_goal: String::new(),
            create_intent: 0,
            intents: Vec::new(),
            root: PathBuf::new(),
            active_work_path: None,
            trace: Vec::new(),
            trace_visible: false,
            detail_visible: false,
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

        self.active_work_path = Some(active_work_path.to_path_buf());

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

    pub(super) fn is_active_work(&self, work: &WorkSummary) -> bool {
        self.active_work_path
            .as_deref()
            .is_some_and(|active_path| active_path == work.path)
    }

    pub(super) fn filtered_count(&self) -> usize {
        self.filtered_indices().len()
    }

    pub(super) fn total_count(&self) -> usize {
        self.works.len()
    }

    pub(super) fn mode_status(&self) -> &'static str {
        match self.mode {
            TuiMode::List => "AWAITING INPUT",
            TuiMode::Search => "SIGNAL FILTER",
            TuiMode::Create => "WORK INIT",
            TuiMode::Archive => "ARCHIVE ACTIVE",
            TuiMode::Help => "KEY INDEX",
        }
    }

    pub(super) fn push_trace(&mut self, kind: TraceKind, message: impl Into<String>) {
        self.trace.insert(
            0,
            TraceEvent {
                kind,
                message: message.into(),
            },
        );
        self.trace.truncate(6);
    }

    pub(super) fn toggle_trace(&mut self) {
        self.trace_visible = !self.trace_visible;
    }

    pub(super) fn toggle_detail(&mut self) {
        self.detail_visible = !self.detail_visible;
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

        if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.toggle_detail();
            self.push_trace(
                TraceKind::Run,
                if self.detail_visible {
                    "detail panel expanded"
                } else {
                    "detail panel compact"
                },
            );
            return TuiAction::None;
        }

        match self.mode {
            TuiMode::List => self.handle_list_key(key),
            TuiMode::Search => self.handle_search_key(key),
            TuiMode::Create => self.handle_create_key(key),
            TuiMode::Archive => self.handle_archive_key(key),
            TuiMode::Help => self.handle_help_key(key),
        }
    }

    pub(super) fn close_panel(&mut self) {
        self.mode = TuiMode::List;
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
                self.push_trace(TraceKind::Signal, "filter ready");
                TuiAction::None
            }
            KeyCode::Char(':') => TuiAction::None,
            KeyCode::Char('n') => {
                self.mode = TuiMode::Create;
                self.push_trace(TraceKind::Run, "work init ready");
                TuiAction::None
            }
            KeyCode::Char('a') => {
                if self.selected_work().is_some() {
                    self.mode = TuiMode::Archive;
                    self.push_trace(TraceKind::Warn, "archive confirmation armed");
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
                self.push_trace(TraceKind::Signal, format!("filter {}", self.filter));
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
                self.push_trace(TraceKind::Signal, filter_trace_message(&self.filter));
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
                self.push_trace(TraceKind::Signal, filter_trace_message(&self.filter));
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

fn filter_trace_message(filter: &str) -> String {
    if filter.is_empty() {
        "filter cleared".to_string()
    } else {
        format!("filter {filter}")
    }
}

fn is_plain_character(key: KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{Toast, TraceKind, TuiAction, TuiMode, TuiState};
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
        let selected = state
            .selected_work()
            .expect("active work should be selected");
        assert!(state.is_active_work(selected));
        assert_eq!(
            state.active_work_path.as_deref(),
            Some(std::path::Path::new(
                "/tmp/workon/.workon/work/review-cache-invalidation-pr"
            ))
        );
        assert_eq!(state.filtered_count(), 3);
        assert_eq!(state.total_count(), 3);
    }

    #[test]
    fn trace_records_newest_operational_events_first_and_caps_history() {
        let mut state = TuiState::new(work_list());

        for index in 0..10 {
            state.push_trace(TraceKind::Run, format!("event {index}"));
        }

        assert_eq!(state.trace.len(), 6);
        assert_eq!(state.trace[0].message, "event 9");
        assert_eq!(state.trace[0].kind, TraceKind::Run);
        assert_eq!(state.trace[5].message, "event 4");
    }

    #[test]
    fn trace_panel_is_hidden_by_default_and_toggles_from_list() {
        let mut state = TuiState::new(work_list());

        assert!(!state.trace_visible);

        let action = state.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));

        assert_eq!(action, TuiAction::None);
        assert!(state.trace_visible);

        let action = state.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));

        assert_eq!(action, TuiAction::None);
        assert!(!state.trace_visible);
    }

    #[test]
    fn detail_panel_is_compact_by_default_and_toggles_globally() {
        let mut state = TuiState::new(work_list());

        assert!(!state.detail_visible);

        let action = state.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));

        assert_eq!(action, TuiAction::None);
        assert!(state.detail_visible);

        let action = state.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));

        assert_eq!(action, TuiAction::None);
        assert!(!state.detail_visible);
    }

    #[test]
    fn clears_toast_on_next_meaningful_key_press() {
        let mut state = TuiState::new(work_list());
        state.toast = Some(Toast::info(
            "Work created",
            "/tmp/workon/.workon/work/billing",
        ));

        let action = state.handle_key(key(KeyCode::Char('j')));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.toast, None);
        assert_eq!(
            state.selected_work().map(|work| work.slug.as_str()),
            Some("review-cache-invalidation-pr")
        );
    }

    #[test]
    fn colon_does_not_open_operator_command() {
        let mut state = TuiState::new(work_list());

        let action = state.handle_key(key(KeyCode::Char(':')));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::List);
        assert!(state.trace.is_empty());
    }

    #[test]
    fn archive_confirm_accepts_enter_and_y() {
        let mut enter_state = TuiState::new(work_list());
        enter_state.mode = TuiMode::Archive;
        let enter_action = enter_state.handle_key(key(KeyCode::Enter));

        let mut y_state = TuiState::new(work_list());
        y_state.mode = TuiMode::Archive;
        let y_action = y_state.handle_key(key(KeyCode::Char('y')));

        assert_eq!(
            enter_action,
            TuiAction::Archive("billing-retry-audit".to_string())
        );
        assert_eq!(y_action, enter_action);
    }

    #[test]
    fn create_intent_navigation_accepts_horizontal_keys() {
        let mut state = TuiState::new(work_list()).with_intents(vec![
            ("investigate".to_string(), "Investigate".to_string()),
            ("review-pr".to_string(), "Review".to_string()),
        ]);
        state.mode = TuiMode::Create;

        assert_eq!(state.handle_key(key(KeyCode::Right)), TuiAction::None);
        assert_eq!(state.create_intent, 1);

        assert_eq!(state.handle_key(key(KeyCode::Left)), TuiAction::None);
        assert_eq!(state.create_intent, 0);

        assert_eq!(state.handle_key(key(KeyCode::BackTab)), TuiAction::None);
        assert_eq!(state.create_intent, 1);
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

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
}
