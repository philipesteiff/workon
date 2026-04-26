use std::collections::BTreeSet;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::{
    AttachedRepository, AvailableRepository, RepositoryCandidate, RepositoryWorkspace,
};

use super::keys::is_plain_character;
use super::state::{Toast, TraceEvent, TraceKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepoPickerState {
    pub(super) focus: RepoPane,
    pub(super) work_slug: String,
    pub(super) work_title: String,
    pub(super) available: Vec<AvailableRepository>,
    pub(super) candidates: Vec<RepositoryCandidate>,
    pub(super) attached: Vec<AttachedRepository>,
    pub(super) workspaces: Vec<RepositoryWorkspace>,
    pub(super) selected_workspace: usize,
    pub(super) selected_catalog: usize,
    workspace_input: String,
    pub(super) filter: String,
    pub(super) pending_add: BTreeSet<String>,
    pub(super) pending_link: BTreeSet<PathBuf>,
    pub(super) pending_remove: BTreeSet<String>,
    pub(super) force_remove: bool,
    pub(super) status: RepoStatus,
    pub(super) logs: Vec<TraceEvent>,
    pub(super) activity_frame: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepoPane {
    Catalog,
    AddPath,
    ConfiguredPaths,
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
    Link,
    Remove,
    Refresh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RepoCatalogRow {
    GitHub(AvailableRepository),
    Local(RepositoryCandidate),
    Attached(AttachedRepository),
}

impl RepoCatalogRow {
    pub(super) fn name(&self) -> &str {
        match self {
            Self::GitHub(repository) => &repository.name_with_owner,
            Self::Local(candidate) => &candidate.name_with_owner,
            Self::Attached(repository) => &repository.name_with_owner,
        }
    }

    pub(super) fn meta(&self) -> String {
        match self {
            Self::GitHub(repository) => format!("github default {}", repository.default_branch),
            Self::Local(candidate) => {
                format!("local {} {}", candidate.branch, candidate.path.display())
            }
            Self::Attached(repository) => format!("attached {}", repository.branch),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RepoPickerAction {
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

impl Default for RepoPickerState {
    fn default() -> Self {
        Self {
            focus: RepoPane::Catalog,
            work_slug: String::new(),
            work_title: String::new(),
            available: Vec::new(),
            candidates: Vec::new(),
            attached: Vec::new(),
            workspaces: Vec::new(),
            selected_workspace: 0,
            selected_catalog: 0,
            workspace_input: String::new(),
            filter: String::new(),
            pending_add: BTreeSet::new(),
            pending_link: BTreeSet::new(),
            pending_remove: BTreeSet::new(),
            force_remove: false,
            status: RepoStatus::Ready,
            logs: Vec::new(),
            activity_frame: 0,
        }
    }
}

impl RepoPickerState {
    #[cfg(test)]
    pub(super) fn enter_context(
        &mut self,
        work_slug: String,
        work_title: String,
        available: Vec<AvailableRepository>,
        candidates: Vec<RepositoryCandidate>,
        attached: Vec<AttachedRepository>,
        workspaces: Vec<RepositoryWorkspace>,
    ) {
        self.focus = if workspaces.is_empty() {
            RepoPane::AddPath
        } else {
            RepoPane::Catalog
        };
        self.work_slug = work_slug;
        self.work_title = work_title;
        self.available = available;
        self.candidates = candidates;
        self.attached = attached;
        self.workspaces = workspaces;
        self.selected_workspace = 0;
        self.selected_catalog = 0;
        self.workspace_input.clear();
        self.filter.clear();
        self.pending_add.clear();
        self.pending_link.clear();
        self.pending_remove.clear();
        self.force_remove = false;
        self.status = RepoStatus::Ready;
    }

    pub(super) fn enter_loading(
        &mut self,
        work_slug: String,
        work_title: String,
        attached: Vec<AttachedRepository>,
    ) {
        self.focus = RepoPane::Catalog;
        self.work_slug = work_slug;
        self.work_title = work_title;
        self.available.clear();
        self.candidates.clear();
        self.attached = attached;
        self.workspaces.clear();
        self.selected_workspace = 0;
        self.selected_catalog = 0;
        self.workspace_input.clear();
        self.filter.clear();
        self.pending_add.clear();
        self.pending_link.clear();
        self.pending_remove.clear();
        self.force_remove = false;
        self.logs.clear();
        self.status = RepoStatus::Loading {
            message: "Loading repository sources".to_string(),
        };
        self.activity_frame = 0;
        self.push_log(TraceKind::Run, "repository context load started");
    }

    pub(super) fn start_loading(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status = RepoStatus::Loading {
            message: message.clone(),
        };
        self.activity_frame = 0;
        self.push_log(TraceKind::Run, message);
    }

    pub(super) fn finish_loading(&mut self) {
        if matches!(self.status, RepoStatus::Loading { .. }) {
            self.status = RepoStatus::Ready;
        }
    }

    pub(super) fn update_attached(&mut self, attached: Vec<AttachedRepository>) {
        self.attached = attached;
        self.pending_add.clear();
        self.pending_link.clear();
        self.pending_remove.clear();
        self.force_remove = false;
        self.status = RepoStatus::Ready;
        self.clamp_selection();
    }

    pub(super) fn update_attached_from_load(&mut self, attached: Vec<AttachedRepository>) {
        self.attached = attached;
        self.pending_add.clear();
        self.pending_link.clear();
        self.pending_remove.clear();
        self.force_remove = false;
        self.clamp_selection();
    }

    pub(super) fn update_available_from_load(&mut self, available: Vec<AvailableRepository>) {
        self.available = available;
        self.clamp_selection();
    }

    pub(super) fn update_workspaces_from_load(&mut self, workspaces: Vec<RepositoryWorkspace>) {
        let was_empty = self.workspaces.is_empty();
        self.workspaces = workspaces;
        self.clamp_selection();
        if was_empty && !self.workspaces.is_empty() && self.focus == RepoPane::AddPath {
            self.focus = RepoPane::Catalog;
        } else if self.workspaces.is_empty() && self.focus == RepoPane::ConfiguredPaths {
            self.focus = RepoPane::AddPath;
        }
    }

    pub(super) fn update_candidates_from_load(&mut self, candidates: Vec<RepositoryCandidate>) {
        self.candidates = candidates;
        self.pending_link.clear();
        self.clamp_selection();
    }

    pub(super) fn set_failed(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status = RepoStatus::Failed {
            message: message.clone(),
        };
        self.push_log(TraceKind::Err, message);
    }

    pub(super) fn start_step(
        &mut self,
        action: RepoOperation,
        current: usize,
        total: usize,
        repository: &str,
    ) {
        self.status = RepoStatus::Applying {
            action,
            current,
            total,
            repository: repository.to_string(),
        };
        self.activity_frame = 0;
        self.push_log(
            TraceKind::Run,
            format!("{} {current}/{total} {repository}", operation_verb(action)),
        );
    }

    pub(super) fn push_log(&mut self, kind: TraceKind, message: impl Into<String>) {
        self.logs.push(TraceEvent {
            kind,
            message: message.into(),
        });
        if self.logs.len() > 7 {
            self.logs.remove(0);
        }
    }

    pub(super) fn advance_activity_frame(&mut self) {
        if self.is_activity_active() {
            self.activity_frame = self.activity_frame.wrapping_add(1);
        }
    }

    pub(super) fn is_activity_active(&self) -> bool {
        matches!(
            self.status,
            RepoStatus::Loading { .. } | RepoStatus::Applying { .. }
        )
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> RepoPickerAction {
        match key.code {
            KeyCode::Esc => {
                self.pending_add.clear();
                self.pending_link.clear();
                self.pending_remove.clear();
                self.force_remove = false;
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

    pub(super) fn catalog_rows(&self) -> Vec<RepoCatalogRow> {
        let query = self.filter.trim().to_ascii_lowercase();
        let local_rows = self
            .candidates
            .iter()
            .filter(|candidate| candidate_matches(candidate, &query))
            .map(|candidate| RepoCatalogRow::Local(candidate.clone()))
            .collect::<Vec<_>>();
        let local_names = local_rows
            .iter()
            .map(|row| row.name().to_string())
            .collect::<BTreeSet<_>>();
        let available_names = self
            .available
            .iter()
            .map(|repository| repository.name_with_owner.clone())
            .collect::<BTreeSet<_>>();
        let attached_names = self
            .attached
            .iter()
            .map(|repository| repository.name_with_owner.clone())
            .collect::<BTreeSet<_>>();

        let mut rows = self
            .available
            .iter()
            .filter(|repository| repository_matches(&repository.name_with_owner, &query))
            .filter(|repository| !local_names.contains(&repository.name_with_owner))
            .map(|repository| RepoCatalogRow::GitHub(repository.clone()))
            .chain(local_rows)
            .chain(
                self.attached
                    .iter()
                    .filter(|repository| {
                        repository_matches(&repository.name_with_owner, &query)
                            && !available_names.contains(&repository.name_with_owner)
                            && !local_names.contains(&repository.name_with_owner)
                    })
                    .map(|repository| RepoCatalogRow::Attached(repository.clone())),
            )
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            let left_attached = attached_names.contains(left.name());
            let right_attached = attached_names.contains(right.name());
            right_attached.cmp(&left_attached).then(
                left.name()
                    .cmp(right.name())
                    .then(left.meta().cmp(&right.meta())),
            )
        });
        rows
    }

    pub(super) fn is_selected(&self, name_with_owner: &str) -> bool {
        if self.pending_remove.contains(name_with_owner) {
            return false;
        }
        self.pending_add.contains(name_with_owner)
            || self
                .attached
                .iter()
                .any(|repository| repository.name_with_owner == name_with_owner)
    }

    pub(super) fn is_local_candidate_selected(&self, path: &PathBuf) -> bool {
        self.pending_link.contains(path)
    }

    pub(super) fn requires_workspace_setup(&self) -> bool {
        self.workspaces.is_empty() && !matches!(self.status, RepoStatus::Loading { .. })
    }

    pub(super) fn workspace_input(&self) -> &str {
        &self.workspace_input
    }

    fn next_focus(&mut self) {
        self.shift_focus(1);
    }

    fn previous_focus(&mut self) {
        self.shift_focus(-1);
    }

    fn shift_focus(&mut self, delta: isize) {
        let order = self.focus_order();
        let current = order
            .iter()
            .position(|pane| *pane == self.focus)
            .unwrap_or_default() as isize;
        let next = (current + delta).rem_euclid(order.len() as isize) as usize;
        self.focus = order[next];
    }

    fn focus_order(&self) -> Vec<RepoPane> {
        vec![
            RepoPane::Catalog,
            RepoPane::AddPath,
            RepoPane::ConfiguredPaths,
        ]
    }

    fn move_selection(&mut self, delta: isize) {
        let count = self.current_count();
        if count == 0 {
            self.set_current_selection(0);
            return;
        }

        let count = count as isize;
        let selected = (self.current_selection() as isize + delta).rem_euclid(count) as usize;
        self.set_current_selection(selected);
    }

    fn toggle_focused_repository(&mut self) {
        match self.focus {
            RepoPane::Catalog => {
                let Some(row) = self.catalog_rows().get(self.selected_catalog).cloned() else {
                    return;
                };
                match row {
                    RepoCatalogRow::GitHub(repository) => {
                        self.toggle_repository_selection(&repository.name_with_owner)
                    }
                    RepoCatalogRow::Local(candidate) => self.toggle_candidate_selection(&candidate),
                    RepoCatalogRow::Attached(repository) => {
                        self.toggle_repository_selection(&repository.name_with_owner)
                    }
                }
            }
            RepoPane::AddPath | RepoPane::ConfiguredPaths => {}
        }
        self.clamp_selection();
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

    pub(super) fn selected_workspace_path(&self) -> Option<PathBuf> {
        self.workspaces
            .get(self.selected_workspace)
            .map(|workspace| workspace.path.clone())
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

    fn toggle_repository_selection(&mut self, name: &str) {
        if self.pending_add.remove(name) {
            return;
        }

        let attached = self
            .attached
            .iter()
            .any(|repository| repository.name_with_owner == name);
        if attached {
            if !self.pending_remove.insert(name.to_string()) {
                self.pending_remove.remove(name);
                if self.pending_remove.is_empty() {
                    self.force_remove = false;
                }
            }
            return;
        }

        if self.pending_remove.remove(name) {
            if self.pending_remove.is_empty() {
                self.force_remove = false;
            }
            return;
        }

        self.remove_pending_links_for(name);
        self.pending_add.insert(name.to_string());
    }

    fn toggle_candidate_selection(&mut self, candidate: &RepositoryCandidate) {
        if self
            .attached
            .iter()
            .any(|repository| repository.name_with_owner == candidate.name_with_owner)
        {
            self.toggle_repository_selection(&candidate.name_with_owner);
            return;
        }

        self.pending_add.remove(&candidate.name_with_owner);
        self.remove_pending_links_for(&candidate.name_with_owner);

        if !self.pending_link.insert(candidate.path.clone()) {
            self.pending_link.remove(&candidate.path);
        }
    }

    fn remove_pending_links_for(&mut self, name_with_owner: &str) {
        let paths = self
            .candidates
            .iter()
            .filter(|candidate| candidate.name_with_owner == name_with_owner)
            .map(|candidate| candidate.path.clone())
            .collect::<Vec<_>>();
        for path in paths {
            self.pending_link.remove(&path);
        }
    }

    fn toggle_force_remove(&mut self) -> RepoPickerAction {
        if self.pending_remove.is_empty() {
            return RepoPickerAction::Notify(Toast::error(
                "No removals selected",
                "Select attached repositories before arming force.",
            ));
        }
        self.force_remove = !self.force_remove;
        self.push_log(
            if self.force_remove {
                TraceKind::Warn
            } else {
                TraceKind::Run
            },
            if self.force_remove {
                "force remove armed"
            } else {
                "force remove disarmed"
            },
        );
        RepoPickerAction::None
    }

    fn current_selection(&self) -> usize {
        match self.focus {
            RepoPane::Catalog => self.selected_catalog,
            RepoPane::AddPath => 0,
            RepoPane::ConfiguredPaths => self.selected_workspace,
        }
    }

    fn set_current_selection(&mut self, selected: usize) {
        match self.focus {
            RepoPane::Catalog => self.selected_catalog = selected,
            RepoPane::AddPath => {}
            RepoPane::ConfiguredPaths => self.selected_workspace = selected,
        }
    }

    fn current_count(&self) -> usize {
        match self.focus {
            RepoPane::Catalog => self.catalog_rows().len(),
            RepoPane::AddPath => 0,
            RepoPane::ConfiguredPaths => self.workspaces.len(),
        }
    }

    fn clamp_selection(&mut self) {
        let catalog_count = self.catalog_rows().len();
        if catalog_count == 0 {
            self.selected_catalog = 0;
        } else if self.selected_catalog >= catalog_count {
            self.selected_catalog = catalog_count - 1;
        }

        let workspace_count = self.workspaces.len();
        if workspace_count == 0 {
            self.selected_workspace = 0;
        } else if self.selected_workspace >= workspace_count {
            self.selected_workspace = workspace_count - 1;
        }
    }
}

fn repository_matches(name_with_owner: &str, query: &str) -> bool {
    query.is_empty() || name_with_owner.to_ascii_lowercase().contains(query)
}

fn candidate_matches(candidate: &RepositoryCandidate, query: &str) -> bool {
    query.is_empty()
        || candidate
            .name_with_owner
            .to_ascii_lowercase()
            .contains(query)
        || candidate.branch.to_ascii_lowercase().contains(query)
        || candidate
            .path
            .display()
            .to_string()
            .to_ascii_lowercase()
            .contains(query)
}

fn operation_verb(action: RepoOperation) -> &'static str {
    match action {
        RepoOperation::Add => "clone",
        RepoOperation::Link => "link",
        RepoOperation::Remove => "remove",
        RepoOperation::Refresh => "refresh",
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

    use crate::domain::{AttachedRepository, AvailableRepository};

    use super::{RepoPane, RepoPickerAction, RepoPickerState};

    #[test]
    fn repo_picker_applies_pending_adds_and_removes() {
        let mut picker = picker();
        picker.handle_key(key(KeyCode::Char(' ')));
        picker.handle_key(key(KeyCode::Down));
        picker.handle_key(key(KeyCode::Char(' ')));

        assert!(matches!(
            picker.handle_key(key(KeyCode::Enter)),
            RepoPickerAction::Apply {
                add,
                remove,
                force_remove: false,
                ..
            } if add == ["openai/api"] && remove == ["openai/workon"]
        ));
    }

    #[test]
    fn repo_picker_arms_force_only_for_pending_removals() {
        let mut picker = picker();
        picker.handle_key(key(KeyCode::Char(' ')));
        picker.handle_key(key(KeyCode::Char('!')));

        assert!(picker.force_remove);
        assert!(picker.pending_remove.contains("openai/workon"));
    }

    #[test]
    fn repo_picker_reports_missing_force_selection_as_notification() {
        let mut picker = picker();

        let action = picker.handle_key(key(KeyCode::Char('!')));

        assert!(matches!(action, RepoPickerAction::Notify(_)));
    }

    #[test]
    fn repo_picker_keeps_attached_repositories_in_catalog_when_sources_are_empty() {
        let mut picker = RepoPickerState::default();
        picker.enter_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            Vec::new(),
            Vec::new(),
            attached_repositories(),
            vec![crate::domain::RepositoryWorkspace {
                path: "/tmp/repos".into(),
            }],
        );

        let rows = picker.catalog_rows();

        assert_eq!(rows.len(), attached_repositories().len());
        assert!(rows.iter().any(|row| row.name() == "openai/workon"));
    }

    #[test]
    fn repo_picker_sorts_attached_repositories_before_unattached_rows() {
        let picker = picker();

        let names = picker
            .catalog_rows()
            .into_iter()
            .map(|row| row.name().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec!["openai/workon".to_string(), "openai/api".to_string()]
        );
    }

    #[test]
    fn repo_picker_focus_cycles_between_repository_workspace_panels() {
        let mut picker = picker();

        picker.handle_key(key(KeyCode::Right));
        assert_eq!(picker.focus, RepoPane::AddPath);

        picker.handle_key(key(KeyCode::Right));
        assert_eq!(picker.focus, RepoPane::ConfiguredPaths);

        picker.handle_key(key(KeyCode::Right));
        assert_eq!(picker.focus, RepoPane::Catalog);
    }

    fn picker() -> RepoPickerState {
        let mut picker = RepoPickerState::default();
        picker.enter_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            Vec::new(),
            attached_repositories(),
            vec![crate::domain::RepositoryWorkspace {
                path: "/tmp/repos".into(),
            }],
        );
        picker
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
                default_branch: "main".to_string(),
                url: "https://github.com/openai/workon".to_string(),
                ssh_url: "git@github.com:openai/workon.git".to_string(),
            },
        ]
    }

    fn attached_repositories() -> Vec<AttachedRepository> {
        vec![AttachedRepository {
            name_with_owner: "openai/workon".to_string(),
            branch: "workon/billing-retry-audit".to_string(),
            path: "/tmp/workon/.workon/work/billing-retry-audit/repos/openai__workon".into(),
            default_branch: "main".to_string(),
            url: "https://github.com/openai/workon".to_string(),
        }]
    }
}
