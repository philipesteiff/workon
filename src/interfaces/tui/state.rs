use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::domain::{
    AttachedRepository, AvailableRepository, IntentCatalog, RepositoryCandidate,
    RepositoryWorkspace, WorkList, WorkSummary,
};

use super::repo_state::{RepoOperation, RepoPickerState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiState {
    pub(super) works: Vec<WorkSummary>,
    pub(super) selected: usize,
    pub(super) mode: TuiMode,
    pub(super) filter: String,
    pub(super) pre_filter_selected_slug: Option<String>,
    pub(super) create_goal: String,
    pub(super) create_intent: usize,
    pub(super) intents: Vec<(String, String)>,
    pub(super) intent_work_slug: String,
    pub(super) intent_work_title: String,
    pub(super) intent_current_id: String,
    pub(super) intent_filter: String,
    pub(super) selected_intent: usize,
    pub(super) repo: RepoPickerState,
    pub(super) root: PathBuf,
    pub(super) current_work_path: Option<PathBuf>,
    pub(super) attached_repositories: BTreeMap<String, Vec<AttachedRepository>>,
    pub(super) repository_index_loading: bool,
    pub(super) repository_index_generation: u64,
    pub(super) trace: Vec<TraceEvent>,
    pub(super) trace_visible: bool,
    pub(super) toast: Option<Toast>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TuiMode {
    List,
    Search,
    Leader,
    Create,
    Archive,
    Intents,
    Repos,
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
    SwitchIntent {
        work_slug: String,
        intent_id: String,
    },
    Create {
        goal: String,
        intent_id: String,
    },
    OpenRepos(String),
    ApplyRepoChanges {
        work_slug: String,
        add: Vec<String>,
        link: Vec<PathBuf>,
        remove: Vec<String>,
        force_remove: bool,
        workspace: Option<PathBuf>,
    },
    AddRepoWorkspaces {
        work_slug: String,
        paths: Vec<PathBuf>,
    },
    RemoveRepoWorkspace {
        work_slug: String,
        path: PathBuf,
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
            intent_work_slug: String::new(),
            intent_work_title: String::new(),
            intent_current_id: String::new(),
            intent_filter: String::new(),
            selected_intent: 0,
            repo: RepoPickerState::default(),
            root: PathBuf::new(),
            current_work_path: None,
            attached_repositories: BTreeMap::new(),
            repository_index_loading: false,
            repository_index_generation: 0,
            trace: Vec::new(),
            trace_visible: false,
            toast: None,
        }
    }

    pub(super) fn with_intents(mut self, intents: Vec<(String, String)>) -> Self {
        self.intents = intents;
        self.reset_create_intent();
        self
    }

    pub(super) fn with_root(mut self, root: PathBuf) -> Self {
        self.root = root;
        self
    }

    #[cfg(test)]
    pub(super) fn with_attached_repositories(
        mut self,
        attached_repositories: BTreeMap<String, Vec<AttachedRepository>>,
    ) -> Self {
        self.attached_repositories = attached_repositories;
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

    pub(super) fn set_attached_repositories(
        &mut self,
        attached_repositories: BTreeMap<String, Vec<AttachedRepository>>,
    ) {
        self.attached_repositories = attached_repositories;
        self.repository_index_loading = false;
    }

    pub(super) fn start_repository_index_loading(&mut self) -> u64 {
        self.repository_index_generation = self.repository_index_generation.wrapping_add(1);
        self.repository_index_loading = true;
        self.repository_index_generation
    }

    pub(super) fn finish_repository_index_loading(&mut self) {
        self.repository_index_loading = false;
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

    pub(super) fn attached_repositories_for(&self, work: &WorkSummary) -> &[AttachedRepository] {
        self.attached_repositories
            .get(&work.slug)
            .map(Vec::as_slice)
            .unwrap_or(&[])
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

    pub(super) fn close_panel(&mut self) {
        self.restore_queue_mode();
    }

    pub(super) fn enter_create_mode(&mut self) {
        self.mode = TuiMode::Create;
        self.reset_create_intent();
    }

    pub(super) fn reset_create_form(&mut self) {
        self.create_goal.clear();
        self.reset_create_intent();
    }

    fn reset_create_intent(&mut self) {
        self.create_intent = self
            .intents
            .iter()
            .position(|(id, _)| id == IntentCatalog::BLANK_INTENT_ID)
            .unwrap_or(0);
    }

    pub(super) fn clear_filter_context(&mut self) {
        self.filter.clear();
        self.pre_filter_selected_slug = None;
    }

    #[cfg(test)]
    pub(super) fn enter_repo_context(
        &mut self,
        work_slug: String,
        work_title: String,
        available: Vec<AvailableRepository>,
        candidates: Vec<RepositoryCandidate>,
        attached: Vec<AttachedRepository>,
        workspaces: Vec<RepositoryWorkspace>,
    ) {
        self.mode = TuiMode::Repos;
        self.attached_repositories
            .insert(work_slug.clone(), attached.clone());
        self.repo.enter_context(
            work_slug, work_title, available, candidates, attached, workspaces,
        );
    }

    pub(super) fn enter_repo_loading(&mut self, work_slug: String, work_title: String) {
        self.mode = TuiMode::Repos;
        let attached = self
            .attached_repositories
            .get(&work_slug)
            .cloned()
            .unwrap_or_default();
        self.repo.enter_loading(work_slug, work_title, attached);
    }

    pub(super) fn advance_activity_frame(&mut self) {
        if self.is_title_activity_active() {
            self.repo.advance_activity_frame();
        }
    }

    pub(super) fn is_title_activity_active(&self) -> bool {
        matches!(self.mode, TuiMode::Repos) && self.repo.is_activity_active()
    }

    pub(super) fn update_attached_repositories(&mut self, attached: Vec<AttachedRepository>) {
        self.repository_index_generation = self.repository_index_generation.wrapping_add(1);
        self.repository_index_loading = false;
        self.attached_repositories
            .insert(self.repo.work_slug.clone(), attached.clone());
        self.repo.update_attached(attached);
    }

    pub(super) fn update_repo_attached_from_load(&mut self, attached: Vec<AttachedRepository>) {
        self.attached_repositories
            .insert(self.repo.work_slug.clone(), attached.clone());
        self.repo.update_attached_from_load(attached);
    }

    pub(super) fn update_repo_available_from_load(&mut self, available: Vec<AvailableRepository>) {
        self.repo.update_available_from_load(available);
    }

    pub(super) fn update_repo_workspaces_from_load(
        &mut self,
        workspaces: Vec<RepositoryWorkspace>,
    ) {
        self.repo.update_workspaces_from_load(workspaces);
    }

    pub(super) fn update_repo_candidates_from_load(
        &mut self,
        candidates: Vec<RepositoryCandidate>,
    ) {
        self.repo.update_candidates_from_load(candidates);
    }

    pub(super) fn start_repo_loading(&mut self, message: impl Into<String>) {
        self.repo.start_loading(message);
    }

    pub(super) fn finish_repo_loading(&mut self) {
        self.repo.finish_loading();
    }

    pub(super) fn set_repo_failed(&mut self, message: impl Into<String>) {
        self.repo.set_failed(message);
    }

    pub(super) fn start_repo_step(
        &mut self,
        action: RepoOperation,
        current: usize,
        total: usize,
        repository: &str,
    ) {
        self.repo.start_step(action, current, total, repository);
    }

    pub(super) fn push_repo_log(&mut self, kind: TraceKind, message: impl Into<String>) {
        self.repo.push_log(kind, message);
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

    pub(super) fn enter_filter_mode(&mut self) {
        if self.filter.is_empty() && self.pre_filter_selected_slug.is_none() {
            self.pre_filter_selected_slug = self.selected_work().map(|work| work.slug.clone());
        }
        self.mode = TuiMode::Search;
    }

    pub(super) fn close_filter_mode(&mut self) {
        let pre_filter_selected_slug = self.pre_filter_selected_slug.take();
        self.filter.clear();
        self.mode = TuiMode::List;

        if let Some(slug) = pre_filter_selected_slug {
            self.select_slug(&slug);
        } else {
            self.clamp_selection();
        }
    }

    pub(super) fn restore_queue_mode(&mut self) {
        self.mode = if self.filter.is_empty() {
            TuiMode::List
        } else {
            TuiMode::Search
        };
    }

    pub(super) fn next_intent(&mut self) {
        if !self.intents.is_empty() {
            self.create_intent = (self.create_intent + 1) % self.intents.len();
        }
    }

    pub(super) fn previous_intent(&mut self) {
        if !self.intents.is_empty() {
            self.create_intent = (self.create_intent + self.intents.len() - 1) % self.intents.len();
        }
    }

    pub(super) fn enter_intent_context(
        &mut self,
        work_slug: String,
        work_title: String,
        current_intent_id: String,
    ) {
        self.mode = TuiMode::Intents;
        self.intent_work_slug = work_slug;
        self.intent_work_title = work_title;
        self.intent_current_id = current_intent_id.clone();
        self.intent_filter.clear();
        self.selected_intent = self
            .filtered_intent_indices()
            .iter()
            .position(|index| {
                self.intents
                    .get(*index)
                    .is_some_and(|(id, _)| id == &current_intent_id)
            })
            .unwrap_or(0);
    }

    pub(super) fn push_intent_filter_char(&mut self, character: char) {
        self.intent_filter.push(character);
        self.selected_intent = 0;
    }

    pub(super) fn pop_intent_filter_char(&mut self) {
        self.intent_filter.pop();
        self.selected_intent = 0;
    }

    pub(super) fn move_intent_selection(&mut self, delta: isize) {
        let count = self.filtered_intent_indices().len();
        if count == 0 {
            self.selected_intent = 0;
            return;
        }

        let count = count as isize;
        self.selected_intent = (self.selected_intent as isize + delta).rem_euclid(count) as usize;
    }

    pub(super) fn selected_intent_id(&self) -> Option<String> {
        let indices = self.filtered_intent_indices();
        indices
            .get(self.selected_intent)
            .and_then(|index| self.intents.get(*index))
            .map(|(id, _)| id.clone())
    }

    pub(super) fn selected_intent_summary(&self) -> Option<(String, String)> {
        let indices = self.filtered_intent_indices();
        indices
            .get(self.selected_intent)
            .and_then(|index| self.intents.get(*index))
            .cloned()
    }

    pub(super) fn filtered_intent_indices(&self) -> Vec<usize> {
        let query = self.intent_filter.trim().to_ascii_lowercase();
        self.intents
            .iter()
            .enumerate()
            .filter_map(|(index, (id, summary))| {
                if query.is_empty()
                    || id.to_ascii_lowercase().contains(&query)
                    || summary.to_ascii_lowercase().contains(&query)
                {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
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
