use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::domain::{AttachedRepository, AvailableRepository, WorkList, WorkSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiState {
    pub(super) works: Vec<WorkSummary>,
    pub(super) selected: usize,
    pub(super) mode: TuiMode,
    pub(super) filter: String,
    pre_filter_selected_slug: Option<String>,
    pub(super) create_goal: String,
    pub(super) create_intent: usize,
    pub(super) intents: Vec<(String, String)>,
    pub(super) repo_focus: RepoPane,
    pub(super) repo_work_slug: String,
    pub(super) repo_work_title: String,
    pub(super) repo_available: Vec<AvailableRepository>,
    pub(super) repo_attached: Vec<AttachedRepository>,
    pub(super) repo_selected: usize,
    pub(super) repo_selected_right: usize,
    pub(super) repo_filter: String,
    pub(super) repo_pending_add: BTreeSet<String>,
    pub(super) repo_pending_remove: BTreeSet<String>,
    pub(super) repo_status: RepoStatus,
    pub(super) repo_logs: Vec<TraceEvent>,
    pub(super) root: PathBuf,
    pub(super) current_work_path: Option<PathBuf>,
    pub(super) trace: Vec<TraceEvent>,
    pub(super) trace_visible: bool,
    pub(super) detail_visible: bool,
    pub(super) toast: Option<Toast>,
    pub(super) activity_frame: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TuiMode {
    List,
    Search,
    Leader,
    Create,
    Archive,
    Repos,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepoPane {
    Catalog,
    Selected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RepoStatus {
    Ready,
    Loading {
        message: String,
    },
    Applying {
        action: RepoOperation,
        current: usize,
        total: usize,
        repository: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepoOperation {
    Add,
    Remove,
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepoSelectionState {
    Attached,
    PendingAdd,
    PendingRemove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedRepositoryRow {
    pub(super) name_with_owner: String,
    pub(super) meta: String,
    pub(super) state: RepoSelectionState,
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
    Create {
        goal: String,
        intent_id: String,
    },
    OpenRepos(String),
    ApplyRepoChanges {
        work_slug: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
}

impl TuiState {
    pub(super) fn new(work_list: WorkList) -> Self {
        Self {
            works: work_list.works,
            selected: 0,
            mode: TuiMode::List,
            filter: String::new(),
            pre_filter_selected_slug: None,
            create_goal: String::new(),
            create_intent: 0,
            intents: Vec::new(),
            repo_focus: RepoPane::Catalog,
            repo_work_slug: String::new(),
            repo_work_title: String::new(),
            repo_available: Vec::new(),
            repo_attached: Vec::new(),
            repo_selected: 0,
            repo_selected_right: 0,
            repo_filter: String::new(),
            repo_pending_add: BTreeSet::new(),
            repo_pending_remove: BTreeSet::new(),
            repo_status: RepoStatus::Ready,
            repo_logs: Vec::new(),
            root: PathBuf::new(),
            current_work_path: None,
            trace: Vec::new(),
            trace_visible: false,
            detail_visible: false,
            toast: None,
            activity_frame: 0,
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

    pub(super) fn with_current_directory(mut self, current_directory: Option<&Path>) -> Self {
        let Some(current_directory) = current_directory else {
            return self;
        };

        if let Some(index) = self
            .works
            .iter()
            .position(|work| current_directory.starts_with(&work.path))
        {
            self.current_work_path = Some(self.works[index].path.clone());
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

    pub(super) fn is_current_work(&self, work: &WorkSummary) -> bool {
        self.current_work_path
            .as_deref()
            .is_some_and(|current_path| current_path == work.path)
    }

    pub(super) fn filtered_count(&self) -> usize {
        self.filtered_indices().len()
    }

    pub(super) fn total_count(&self) -> usize {
        self.works.len()
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
            TuiMode::Leader => self.handle_leader_key(key),
            TuiMode::Create => self.handle_create_key(key),
            TuiMode::Archive => self.handle_archive_key(key),
            TuiMode::Repos => self.handle_repos_key(key),
            TuiMode::Help => self.handle_help_key(key),
        }
    }

    pub(super) fn close_panel(&mut self) {
        self.restore_queue_mode();
    }

    pub(super) fn reset_create_form(&mut self) {
        self.create_goal.clear();
        self.create_intent = 0;
    }

    pub(super) fn clear_filter_context(&mut self) {
        self.filter.clear();
        self.pre_filter_selected_slug = None;
    }

    pub(super) fn enter_repo_context(
        &mut self,
        work_slug: String,
        work_title: String,
        available: Vec<AvailableRepository>,
        attached: Vec<AttachedRepository>,
    ) {
        self.mode = TuiMode::Repos;
        self.repo_focus = RepoPane::Catalog;
        self.repo_work_slug = work_slug;
        self.repo_work_title = work_title;
        self.repo_available = available;
        self.repo_attached = attached;
        self.repo_selected = 0;
        self.repo_selected_right = 0;
        self.repo_filter.clear();
        self.repo_pending_add.clear();
        self.repo_pending_remove.clear();
        self.repo_status = RepoStatus::Ready;
    }

    pub(super) fn enter_repo_loading(&mut self, work_slug: String, work_title: String) {
        self.mode = TuiMode::Repos;
        self.repo_focus = RepoPane::Catalog;
        self.repo_work_slug = work_slug;
        self.repo_work_title = work_title;
        self.repo_available.clear();
        self.repo_attached.clear();
        self.repo_selected = 0;
        self.repo_selected_right = 0;
        self.repo_filter.clear();
        self.repo_pending_add.clear();
        self.repo_pending_remove.clear();
        self.repo_logs.clear();
        self.repo_status = RepoStatus::Loading {
            message: "Loading GitHub repositories".to_string(),
        };
        self.activity_frame = 0;
        self.push_repo_log(TraceKind::Run, "gh repo list started");
    }

    pub(super) fn advance_activity_frame(&mut self) {
        if self.is_title_activity_active() {
            self.activity_frame = self.activity_frame.wrapping_add(1);
        }
    }

    pub(super) fn is_title_activity_active(&self) -> bool {
        matches!(self.mode, TuiMode::Repos)
            && matches!(self.repo_status, RepoStatus::Loading { .. })
    }

    pub(super) fn update_attached_repositories(&mut self, attached: Vec<AttachedRepository>) {
        self.repo_attached = attached;
        self.repo_pending_add.clear();
        self.repo_pending_remove.clear();
        self.repo_status = RepoStatus::Ready;
        self.clamp_repo_selection();
        self.clamp_selected_repo_selection();
    }

    pub(super) fn set_repo_failed(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.repo_status = RepoStatus::Failed {
            message: message.clone(),
        };
        self.push_repo_log(TraceKind::Err, message);
    }

    pub(super) fn start_repo_step(
        &mut self,
        action: RepoOperation,
        current: usize,
        total: usize,
        repository: &str,
    ) {
        self.repo_status = RepoStatus::Applying {
            action,
            current,
            total,
            repository: repository.to_string(),
        };
        self.push_repo_log(
            TraceKind::Run,
            format!(
                "{} {current}/{total} {repository}",
                repo_operation_verb(action)
            ),
        );
    }

    pub(super) fn push_repo_log(&mut self, kind: TraceKind, message: impl Into<String>) {
        self.repo_logs.push(TraceEvent {
            kind,
            message: message.into(),
        });
        if self.repo_logs.len() > 7 {
            self.repo_logs.remove(0);
        }
    }

    pub(super) fn filtered_available_repositories(&self) -> Vec<&AvailableRepository> {
        let query = self.repo_filter.trim().to_ascii_lowercase();
        let mut repositories = self
            .repo_available
            .iter()
            .filter(|repository| repository_matches(&repository.name_with_owner, &query))
            .collect::<Vec<_>>();
        repositories.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
        repositories
    }

    pub(super) fn selected_repository_rows(&self) -> Vec<SelectedRepositoryRow> {
        let mut rows = self
            .repo_attached
            .iter()
            .map(|repository| SelectedRepositoryRow {
                name_with_owner: repository.name_with_owner.clone(),
                meta: repository.branch.clone(),
                state: if self
                    .repo_pending_remove
                    .contains(&repository.name_with_owner)
                {
                    RepoSelectionState::PendingRemove
                } else {
                    RepoSelectionState::Attached
                },
            })
            .collect::<Vec<_>>();

        let attached = self.attached_repository_names();
        for repository in &self.repo_available {
            if self.repo_pending_add.contains(&repository.name_with_owner)
                && !attached.contains(&repository.name_with_owner)
            {
                rows.push(SelectedRepositoryRow {
                    name_with_owner: repository.name_with_owner.clone(),
                    meta: format!("default {}", repository.default_branch),
                    state: RepoSelectionState::PendingAdd,
                });
            }
        }

        rows.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
        rows
    }

    pub(super) fn is_repository_selected(&self, name_with_owner: &str) -> bool {
        if self.repo_pending_remove.contains(name_with_owner) {
            return false;
        }
        self.repo_pending_add.contains(name_with_owner)
            || self
                .repo_attached
                .iter()
                .any(|repository| repository.name_with_owner == name_with_owner)
    }

    pub(super) fn pending_repo_change_count(&self) -> usize {
        self.repo_pending_add.len() + self.repo_pending_remove.len()
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
                self.push_trace(TraceKind::Signal, format!("filter {}", self.filter));
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
        match key.code {
            KeyCode::Esc => {
                self.repo_pending_add.clear();
                self.repo_pending_remove.clear();
                self.repo_filter.clear();
                self.restore_queue_mode();
                TuiAction::None
            }
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => {
                self.toggle_repo_focus();
                TuiAction::None
            }
            KeyCode::Down => {
                self.move_repo_selection(1);
                TuiAction::None
            }
            KeyCode::Up => {
                self.move_repo_selection(-1);
                TuiAction::None
            }
            KeyCode::Backspace if !self.repo_filter.is_empty() => {
                self.repo_filter.pop();
                self.repo_selected = 0;
                TuiAction::None
            }
            KeyCode::Char(' ') if is_plain_character(key) => {
                self.toggle_focused_repository();
                TuiAction::None
            }
            KeyCode::Enter => self.repo_apply_action(),
            KeyCode::Char(character) if is_plain_character(key) => {
                self.repo_focus = RepoPane::Catalog;
                self.repo_filter.push(character);
                self.repo_selected = 0;
                TuiAction::None
            }
            _ => TuiAction::None,
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

    fn enter_filter_mode(&mut self) {
        if self.filter.is_empty() && self.pre_filter_selected_slug.is_none() {
            self.pre_filter_selected_slug = self.selected_work().map(|work| work.slug.clone());
        }
        self.mode = TuiMode::Search;
    }

    fn close_filter_mode(&mut self) {
        let pre_filter_selected_slug = self.pre_filter_selected_slug.take();
        self.filter.clear();
        self.mode = TuiMode::List;

        if let Some(slug) = pre_filter_selected_slug {
            self.select_slug(&slug);
        } else {
            self.clamp_selection();
        }
    }

    fn restore_queue_mode(&mut self) {
        self.mode = if self.filter.is_empty() {
            TuiMode::List
        } else {
            TuiMode::Search
        };
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

    fn toggle_repo_focus(&mut self) {
        self.repo_focus = match self.repo_focus {
            RepoPane::Catalog => RepoPane::Selected,
            RepoPane::Selected => RepoPane::Catalog,
        };
    }

    fn move_repo_selection(&mut self, delta: isize) {
        let count = self.current_repo_count();
        if count == 0 {
            self.set_current_repo_selection(0);
            return;
        }

        let count = count as isize;
        let selected = (self.current_repo_selection() as isize + delta).rem_euclid(count) as usize;
        self.set_current_repo_selection(selected);
    }

    fn toggle_focused_repository(&mut self) {
        match self.repo_focus {
            RepoPane::Catalog => {
                let Some(name) = self.selected_catalog_repo_name() else {
                    return;
                };
                self.toggle_repository_selection(&name);
            }
            RepoPane::Selected => {
                let Some(name) = self.selected_work_repo_name() else {
                    return;
                };
                self.toggle_repository_selection(&name);
            }
        }
        self.clamp_selected_repo_selection();
    }

    fn repo_apply_action(&mut self) -> TuiAction {
        if self.repo_pending_add.is_empty() && self.repo_pending_remove.is_empty() {
            return TuiAction::None;
        }

        TuiAction::ApplyRepoChanges {
            work_slug: self.repo_work_slug.clone(),
            add: self.repo_pending_add.iter().cloned().collect(),
            remove: self.repo_pending_remove.iter().cloned().collect(),
        }
    }

    fn toggle_repository_selection(&mut self, name: &str) {
        if self.repo_pending_add.remove(name) {
            return;
        }

        let attached = self
            .repo_attached
            .iter()
            .any(|repository| repository.name_with_owner == name);
        if attached {
            if !self.repo_pending_remove.insert(name.to_string()) {
                self.repo_pending_remove.remove(name);
            }
            return;
        }

        if self.repo_pending_remove.remove(name) {
            return;
        }

        self.repo_pending_add.insert(name.to_string());
    }

    fn selected_catalog_repo_name(&self) -> Option<String> {
        self.filtered_available_repositories()
            .get(self.repo_selected)
            .map(|repository| repository.name_with_owner.clone())
    }

    fn selected_work_repo_name(&self) -> Option<String> {
        self.selected_repository_rows()
            .get(self.repo_selected_right)
            .map(|repository| repository.name_with_owner.clone())
    }

    fn current_repo_selection(&self) -> usize {
        match self.repo_focus {
            RepoPane::Catalog => self.repo_selected,
            RepoPane::Selected => self.repo_selected_right,
        }
    }

    fn set_current_repo_selection(&mut self, selected: usize) {
        match self.repo_focus {
            RepoPane::Catalog => self.repo_selected = selected,
            RepoPane::Selected => self.repo_selected_right = selected,
        }
    }

    fn current_repo_count(&self) -> usize {
        match self.repo_focus {
            RepoPane::Catalog => self.filtered_available_repositories().len(),
            RepoPane::Selected => self.selected_repository_rows().len(),
        }
    }

    fn clamp_repo_selection(&mut self) {
        let count = self.current_repo_count();
        if count == 0 {
            self.repo_selected = 0;
        } else if self.repo_selected >= count {
            self.repo_selected = count - 1;
        }
    }

    fn clamp_selected_repo_selection(&mut self) {
        let count = self.selected_repository_rows().len();
        if count == 0 {
            self.repo_selected_right = 0;
        } else if self.repo_selected_right >= count {
            self.repo_selected_right = count - 1;
        }
    }

    fn attached_repository_names(&self) -> BTreeSet<String> {
        self.repo_attached
            .iter()
            .map(|repository| repository.name_with_owner.clone())
            .collect()
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

fn repository_matches(name_with_owner: &str, query: &str) -> bool {
    query.is_empty() || name_with_owner.to_ascii_lowercase().contains(query)
}

fn repo_operation_verb(action: RepoOperation) -> &'static str {
    match action {
        RepoOperation::Add => "clone",
        RepoOperation::Remove => "remove",
        RepoOperation::Refresh => "refresh",
    }
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

fn is_shortcut_character(key: KeyEvent) -> bool {
    key.modifiers == KeyModifiers::NONE
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{RepoPane, RepoStatus, Toast, TraceKind, TuiAction, TuiMode, TuiState};
    use crate::domain::{AttachedRepository, AvailableRepository, WorkList, WorkSummary};

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
    fn selects_current_work_when_cwd_is_inside_work_folder() {
        let state = TuiState::new(work_list()).with_current_directory(Some(std::path::Path::new(
            "/tmp/workon/.workon/work/review-cache-invalidation-pr/src",
        )));

        assert_eq!(
            state.selected_work().map(|work| work.slug.as_str()),
            Some("review-cache-invalidation-pr")
        );
        let selected = state
            .selected_work()
            .expect("current work should be selected");
        assert!(state.is_current_work(selected));
        assert_eq!(
            state.current_work_path.as_deref(),
            Some(std::path::Path::new(
                "/tmp/workon/.workon/work/review-cache-invalidation-pr"
            ))
        );
        assert_eq!(state.filtered_count(), 3);
        assert_eq!(state.total_count(), 3);
    }

    #[test]
    fn leaves_all_work_active_when_cwd_is_outside_work_folders() {
        let state = TuiState::new(work_list())
            .with_current_directory(Some(std::path::Path::new("/tmp/workon/elsewhere")));

        assert_eq!(
            state.selected_work().map(|work| work.slug.as_str()),
            Some("billing-retry-audit")
        );
        assert!(state.current_work_path.is_none());
        assert!(state.works.iter().all(|work| !state.is_current_work(work)));
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

        let action = state.handle_key(key(KeyCode::Down));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.toast, None);
        assert_eq!(
            state.selected_work().map(|work| work.slug.as_str()),
            Some("review-cache-invalidation-pr")
        );
    }

    #[test]
    fn colon_starts_filter_like_other_printable_characters() {
        let mut state = TuiState::new(work_list());

        let action = state.handle_key(key(KeyCode::Char(':')));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::Search);
        assert_eq!(state.filter, ":");
        assert_eq!(state.trace[0].message, "filter :");
    }

    #[test]
    fn direct_typing_starts_filtering_from_list_mode() {
        let mut state = TuiState::new(work_list());
        state.move_selection(1);

        let mut action = TuiAction::None;
        for character in "billing".chars() {
            action = state.handle_key(key(KeyCode::Char(character)));
        }

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::Search);
        assert_eq!(state.filter, "billing");
        assert_eq!(state.filtered_count(), 1);
        assert_eq!(
            state.selected_work().map(|work| work.slug.as_str()),
            Some("billing-retry-audit")
        );
    }

    #[test]
    fn bare_printable_shortcuts_append_to_filter() {
        for character in ['j', 'k', 'n', 'a', 'q', '?', ' '] {
            let mut state = TuiState::new(work_list());

            let action = state.handle_key(key(KeyCode::Char(character)));

            assert_eq!(action, TuiAction::None);
            assert_eq!(state.mode, TuiMode::Search);
            assert_eq!(state.filter, character.to_string());
        }
    }

    #[test]
    fn slash_enters_leader_without_changing_filter_or_selection() {
        let mut state = TuiState::new(work_list());
        state.move_selection(1);
        let selected_before = state.selected_work().map(|work| work.slug.clone());

        let action = state.handle_key(key(KeyCode::Char('/')));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::Leader);
        assert_eq!(state.filter, "");
        assert_eq!(state.filtered_count(), 3);
        assert_eq!(
            state.selected_work().map(|work| work.slug.clone()),
            selected_before
        );
    }

    #[test]
    fn enter_in_filter_switches_highlighted_match() {
        let mut state = TuiState::new(work_list());
        for character in "context".chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }

        let action = state.handle_key(key(KeyCode::Enter));

        assert_eq!(
            action,
            TuiAction::Switch("context-command-sketch".to_string())
        );
    }

    #[test]
    fn search_navigation_switches_directly_from_filtered_queue() {
        let mut state = TuiState::new(work_list());
        state.handle_key(key(KeyCode::Char('i')));
        state.handle_key(key(KeyCode::Char('n')));
        state.handle_key(key(KeyCode::Char('v')));
        state.handle_key(key(KeyCode::Down));

        let action = state.handle_key(key(KeyCode::Enter));

        assert_eq!(
            action,
            TuiAction::Switch("review-cache-invalidation-pr".to_string())
        );
    }

    #[test]
    fn slash_leader_commands_route_actions() {
        let mut state = TuiState::new(work_list());
        state.handle_key(key(KeyCode::Char('/')));
        assert_eq!(state.handle_key(key(KeyCode::Char('n'))), TuiAction::None);
        assert_eq!(state.mode, TuiMode::Create);

        let mut archive_state = TuiState::new(work_list());
        for character in "review".chars() {
            archive_state.handle_key(key(KeyCode::Char(character)));
        }
        archive_state.handle_key(key(KeyCode::Char('/')));
        assert_eq!(
            archive_state.handle_key(key(KeyCode::Char('a'))),
            TuiAction::None
        );
        assert_eq!(archive_state.mode, TuiMode::Archive);
        assert_eq!(
            archive_state.handle_key(key(KeyCode::Enter)),
            TuiAction::Archive("review-cache-invalidation-pr".to_string())
        );

        let mut help_state = TuiState::new(work_list());
        help_state.handle_key(key(KeyCode::Char('/')));
        assert_eq!(
            help_state.handle_key(key(KeyCode::Char('?'))),
            TuiAction::None
        );
        assert_eq!(help_state.mode, TuiMode::Help);

        let mut quit_state = TuiState::new(work_list());
        quit_state.handle_key(key(KeyCode::Char('/')));
        assert_eq!(
            quit_state.handle_key(key(KeyCode::Char('q'))),
            TuiAction::Quit
        );
    }

    #[test]
    fn slash_leader_opens_repo_context_for_highlighted_work() {
        let mut state = TuiState::new(work_list());
        state.move_selection(1);

        state.handle_key(key(KeyCode::Char('/')));
        let action = state.handle_key(key(KeyCode::Char('r')));

        assert_eq!(
            action,
            TuiAction::OpenRepos("review-cache-invalidation-pr".to_string())
        );
        assert_eq!(state.mode, TuiMode::Repos);
        assert_eq!(state.repo_focus, RepoPane::Catalog);
        assert_eq!(
            state.repo_status,
            RepoStatus::Loading {
                message: "Loading GitHub repositories".to_string()
            }
        );
    }

    #[test]
    fn repo_context_keeps_github_catalog_visible_with_attached_repositories() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            attached_repositories(),
        );

        assert_eq!(state.mode, TuiMode::Repos);
        let names = state
            .filtered_available_repositories()
            .into_iter()
            .map(|repository| repository.name_with_owner.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "openai/api".to_string(),
                "openai/api-docs".to_string(),
                "openai/workon".to_string(),
            ]
        );
    }

    #[test]
    fn repo_context_applies_pending_adds_and_removes_from_two_panel_picker() {
        let mut state = TuiState::new(work_list());
        state.enter_repo_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            attached_repositories(),
        );

        assert_eq!(state.repo_focus, RepoPane::Catalog);
        state.handle_key(key(KeyCode::Char(' ')));
        state.handle_key(key(KeyCode::Down));
        state.handle_key(key(KeyCode::Char(' ')));

        assert_eq!(
            state.handle_key(key(KeyCode::Enter)),
            TuiAction::ApplyRepoChanges {
                work_slug: "billing-retry-audit".to_string(),
                add: vec!["openai/api-docs".to_string()],
                remove: vec!["openai/api".to_string()],
            }
        );
    }

    #[test]
    fn slash_leader_can_insert_literal_slash_and_cancel() {
        let mut state = TuiState::new(work_list());
        state.handle_key(key(KeyCode::Char('b')));
        state.handle_key(key(KeyCode::Char('/')));

        let action = state.handle_key(key(KeyCode::Esc));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::Search);
        assert_eq!(state.filter, "b");

        state.handle_key(key(KeyCode::Char('/')));
        let action = state.handle_key(key(KeyCode::Char('/')));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::Search);
        assert_eq!(state.filter, "b/");
    }

    #[test]
    fn escape_clears_filter_and_restores_previous_selection() {
        let mut state = TuiState::new(work_list());
        state.move_selection(1);
        let selected_before = state.selected_work().map(|work| work.slug.clone());
        for character in "context".chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }

        let action = state.handle_key(key(KeyCode::Esc));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::List);
        assert_eq!(state.filter, "");
        assert_eq!(
            state.selected_work().map(|work| work.slug.clone()),
            selected_before
        );
    }

    #[test]
    fn clearing_filter_context_allows_next_filter_to_restore_current_selection() {
        let mut state = TuiState::new(work_list());
        state.move_selection(1);
        for character in "context".chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }

        state.clear_filter_context();
        state.select_slug("billing-retry-audit");
        state.close_panel();
        for character in "review".chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }

        let action = state.handle_key(key(KeyCode::Esc));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::List);
        assert_eq!(state.filter, "");
        assert_eq!(
            state.selected_work().map(|work| work.slug.as_str()),
            Some("billing-retry-audit")
        );
    }

    #[test]
    fn enter_on_empty_filter_match_does_not_switch_or_close_filter() {
        let mut state = TuiState::new(work_list());
        for character in "missing".chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }

        let action = state.handle_key(key(KeyCode::Enter));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::Search);
        assert_eq!(state.filter, "missing");
    }

    #[test]
    fn backspace_edits_filter_and_resets_selection() {
        let mut state = TuiState::new(work_list());
        for character in "review".chars() {
            state.handle_key(key(KeyCode::Char(character)));
        }
        state.handle_key(key(KeyCode::Down));

        let action = state.handle_key(key(KeyCode::Backspace));

        assert_eq!(action, TuiAction::None);
        assert_eq!(state.mode, TuiMode::Search);
        assert_eq!(state.filter, "revie");
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn escape_with_empty_filter_quits() {
        let mut state = TuiState::new(work_list());

        assert_eq!(state.handle_key(key(KeyCode::Esc)), TuiAction::Quit);
    }

    #[test]
    fn modified_list_shortcuts_are_ignored() {
        for (code, modifiers) in [
            (KeyCode::Char('q'), KeyModifiers::ALT),
            (KeyCode::Char('j'), KeyModifiers::CONTROL),
            (KeyCode::Char('k'), KeyModifiers::ALT),
            (KeyCode::Char('n'), KeyModifiers::ALT),
            (KeyCode::Char('a'), KeyModifiers::CONTROL),
            (KeyCode::Char('?'), KeyModifiers::ALT),
            (KeyCode::Char('/'), KeyModifiers::CONTROL),
        ] {
            let mut state = TuiState::new(work_list());

            let action = state.handle_key(KeyEvent::new(code, modifiers));

            assert_eq!(action, TuiAction::None);
            assert_eq!(state.mode, TuiMode::List);
            assert_eq!(state.selected, 0);
            assert!(state.trace.is_empty());
        }
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

    fn available_repositories() -> Vec<AvailableRepository> {
        vec![
            AvailableRepository {
                name_with_owner: "openai/api".to_string(),
                default_branch: "main".to_string(),
                url: "https://github.com/openai/api".to_string(),
                ssh_url: "git@github.com:openai/api.git".to_string(),
            },
            AvailableRepository {
                name_with_owner: "openai/workon".to_string(),
                default_branch: "trunk".to_string(),
                url: "https://github.com/openai/workon".to_string(),
                ssh_url: "git@github.com:openai/workon.git".to_string(),
            },
            AvailableRepository {
                name_with_owner: "openai/api-docs".to_string(),
                default_branch: "main".to_string(),
                url: "https://github.com/openai/api-docs".to_string(),
                ssh_url: "git@github.com:openai/api-docs.git".to_string(),
            },
        ]
    }

    fn attached_repositories() -> Vec<AttachedRepository> {
        vec![
            AttachedRepository {
                name_with_owner: "openai/api".to_string(),
                branch: "workon/billing-retry-audit".to_string(),
                path: PathBuf::from(
                    "/tmp/workon/.workon/work/billing-retry-audit/repos/openai__api",
                ),
                default_branch: "main".to_string(),
                url: "https://github.com/openai/api".to_string(),
            },
            AttachedRepository {
                name_with_owner: "openai/workon".to_string(),
                branch: "workon/billing-retry-audit".to_string(),
                path: PathBuf::from(
                    "/tmp/workon/.workon/work/billing-retry-audit/repos/openai__workon",
                ),
                default_branch: "trunk".to_string(),
                url: "https://github.com/openai/workon".to_string(),
            },
        ]
    }
}
