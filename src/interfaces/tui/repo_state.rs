use std::collections::BTreeSet;

use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::{AttachedRepository, AvailableRepository};

use super::keys::is_plain_character;
use super::state::{Toast, TraceEvent, TraceKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepoPickerState {
    pub(super) focus: RepoPane,
    pub(super) work_slug: String,
    pub(super) work_title: String,
    pub(super) available: Vec<AvailableRepository>,
    pub(super) attached: Vec<AttachedRepository>,
    pub(super) selected_catalog: usize,
    pub(super) selected_work: usize,
    pub(super) filter: String,
    pub(super) pending_add: BTreeSet<String>,
    pub(super) pending_remove: BTreeSet<String>,
    pub(super) force_remove: bool,
    pub(super) status: RepoStatus,
    pub(super) logs: Vec<TraceEvent>,
    pub(super) activity_frame: usize,
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
    PendingForceRemove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedRepositoryRow {
    pub(super) name_with_owner: String,
    pub(super) meta: String,
    pub(super) state: RepoSelectionState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RepoPickerAction {
    None,
    Back,
    Notify(Toast),
    Apply {
        work_slug: String,
        add: Vec<String>,
        remove: Vec<String>,
        force_remove: bool,
    },
}

impl Default for RepoPickerState {
    fn default() -> Self {
        Self {
            focus: RepoPane::Catalog,
            work_slug: String::new(),
            work_title: String::new(),
            available: Vec::new(),
            attached: Vec::new(),
            selected_catalog: 0,
            selected_work: 0,
            filter: String::new(),
            pending_add: BTreeSet::new(),
            pending_remove: BTreeSet::new(),
            force_remove: false,
            status: RepoStatus::Ready,
            logs: Vec::new(),
            activity_frame: 0,
        }
    }
}

impl RepoPickerState {
    pub(super) fn enter_context(
        &mut self,
        work_slug: String,
        work_title: String,
        available: Vec<AvailableRepository>,
        attached: Vec<AttachedRepository>,
    ) {
        self.focus = RepoPane::Catalog;
        self.work_slug = work_slug;
        self.work_title = work_title;
        self.available = available;
        self.attached = attached;
        self.selected_catalog = 0;
        self.selected_work = 0;
        self.filter.clear();
        self.pending_add.clear();
        self.pending_remove.clear();
        self.force_remove = false;
        self.status = RepoStatus::Ready;
    }

    pub(super) fn enter_loading(&mut self, work_slug: String, work_title: String) {
        self.focus = RepoPane::Catalog;
        self.work_slug = work_slug;
        self.work_title = work_title;
        self.available.clear();
        self.attached.clear();
        self.selected_catalog = 0;
        self.selected_work = 0;
        self.filter.clear();
        self.pending_add.clear();
        self.pending_remove.clear();
        self.force_remove = false;
        self.logs.clear();
        self.status = RepoStatus::Loading {
            message: "Loading GitHub repositories".to_string(),
        };
        self.activity_frame = 0;
        self.push_log(TraceKind::Run, "gh repo list started");
    }

    pub(super) fn update_attached(&mut self, attached: Vec<AttachedRepository>) {
        self.attached = attached;
        self.pending_add.clear();
        self.pending_remove.clear();
        self.force_remove = false;
        self.status = RepoStatus::Ready;
        self.clamp_selection();
        self.clamp_selected_selection();
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
                self.pending_remove.clear();
                self.force_remove = false;
                self.filter.clear();
                RepoPickerAction::Back
            }
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => {
                self.toggle_focus();
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
            KeyCode::Backspace if !self.filter.is_empty() => {
                self.filter.pop();
                self.selected_catalog = 0;
                RepoPickerAction::None
            }
            KeyCode::Char(' ') if is_plain_character(key) => {
                self.toggle_focused_repository();
                RepoPickerAction::None
            }
            KeyCode::Char('!') if is_plain_character(key) => self.toggle_force_remove(),
            KeyCode::Enter => self.apply_action(),
            KeyCode::Char(character) if is_plain_character(key) => {
                self.focus = RepoPane::Catalog;
                self.filter.push(character);
                self.selected_catalog = 0;
                RepoPickerAction::None
            }
            _ => RepoPickerAction::None,
        }
    }

    pub(super) fn filtered_available(&self) -> Vec<&AvailableRepository> {
        let query = self.filter.trim().to_ascii_lowercase();
        let mut repositories = self
            .available
            .iter()
            .filter(|repository| repository_matches(&repository.name_with_owner, &query))
            .collect::<Vec<_>>();
        repositories.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
        repositories
    }

    pub(super) fn selected_rows(&self) -> Vec<SelectedRepositoryRow> {
        let mut rows = self
            .attached
            .iter()
            .map(|repository| SelectedRepositoryRow {
                name_with_owner: repository.name_with_owner.clone(),
                meta: repository.branch.clone(),
                state: if self.pending_remove.contains(&repository.name_with_owner) {
                    if self.force_remove {
                        RepoSelectionState::PendingForceRemove
                    } else {
                        RepoSelectionState::PendingRemove
                    }
                } else {
                    RepoSelectionState::Attached
                },
            })
            .collect::<Vec<_>>();

        let attached = self.attached_repository_names();
        for repository in &self.available {
            if self.pending_add.contains(&repository.name_with_owner)
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

    pub(super) fn pending_change_count(&self) -> usize {
        self.pending_add.len() + self.pending_remove.len()
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            RepoPane::Catalog => RepoPane::Selected,
            RepoPane::Selected => RepoPane::Catalog,
        };
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
        self.clamp_selected_selection();
    }

    fn apply_action(&mut self) -> RepoPickerAction {
        if self.pending_add.is_empty() && self.pending_remove.is_empty() {
            return RepoPickerAction::None;
        }

        RepoPickerAction::Apply {
            work_slug: self.work_slug.clone(),
            add: self.pending_add.iter().cloned().collect(),
            remove: self.pending_remove.iter().cloned().collect(),
            force_remove: self.force_remove && !self.pending_remove.is_empty(),
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

        self.pending_add.insert(name.to_string());
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

    fn selected_catalog_repo_name(&self) -> Option<String> {
        self.filtered_available()
            .get(self.selected_catalog)
            .map(|repository| repository.name_with_owner.clone())
    }

    fn selected_work_repo_name(&self) -> Option<String> {
        self.selected_rows()
            .get(self.selected_work)
            .map(|repository| repository.name_with_owner.clone())
    }

    fn current_selection(&self) -> usize {
        match self.focus {
            RepoPane::Catalog => self.selected_catalog,
            RepoPane::Selected => self.selected_work,
        }
    }

    fn set_current_selection(&mut self, selected: usize) {
        match self.focus {
            RepoPane::Catalog => self.selected_catalog = selected,
            RepoPane::Selected => self.selected_work = selected,
        }
    }

    fn current_count(&self) -> usize {
        match self.focus {
            RepoPane::Catalog => self.filtered_available().len(),
            RepoPane::Selected => self.selected_rows().len(),
        }
    }

    fn clamp_selection(&mut self) {
        let count = self.current_count();
        if count == 0 {
            self.selected_catalog = 0;
        } else if self.selected_catalog >= count {
            self.selected_catalog = count - 1;
        }
    }

    fn clamp_selected_selection(&mut self) {
        let count = self.selected_rows().len();
        if count == 0 {
            self.selected_work = 0;
        } else if self.selected_work >= count {
            self.selected_work = count - 1;
        }
    }

    fn attached_repository_names(&self) -> BTreeSet<String> {
        self.attached
            .iter()
            .map(|repository| repository.name_with_owner.clone())
            .collect()
    }
}

fn repository_matches(name_with_owner: &str, query: &str) -> bool {
    query.is_empty() || name_with_owner.to_ascii_lowercase().contains(query)
}

fn operation_verb(action: RepoOperation) -> &'static str {
    match action {
        RepoOperation::Add => "clone",
        RepoOperation::Remove => "remove",
        RepoOperation::Refresh => "refresh",
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::domain::{AttachedRepository, AvailableRepository};

    use super::{RepoPane, RepoPickerAction, RepoPickerState, RepoSelectionState};

    #[test]
    fn repo_picker_applies_pending_adds_and_removes() {
        let mut picker = picker();
        picker.handle_key(key(KeyCode::Char(' ')));
        picker.handle_key(key(KeyCode::Right));
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
        picker.focus = RepoPane::Selected;
        picker.selected_work = 0;
        picker.handle_key(key(KeyCode::Char(' ')));
        picker.handle_key(key(KeyCode::Char('!')));

        assert!(picker.force_remove);
        assert_eq!(
            picker.selected_rows()[0].state,
            RepoSelectionState::PendingForceRemove
        );
    }

    #[test]
    fn repo_picker_reports_missing_force_selection_as_notification() {
        let mut picker = picker();

        let action = picker.handle_key(key(KeyCode::Char('!')));

        assert!(matches!(action, RepoPickerAction::Notify(_)));
    }

    fn picker() -> RepoPickerState {
        let mut picker = RepoPickerState::default();
        picker.enter_context(
            "billing-retry-audit".to_string(),
            "Billing retry audit".to_string(),
            available_repositories(),
            attached_repositories(),
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
