use crate::domain::{
    AttachedRepository, AvailableRepository, RepositoryCandidate, RepositoryWorkspace,
};

use super::status::operation_verb;
use super::{RepoOperation, RepoPane, RepoPickerState, RepoStatus};
use crate::interfaces::tui::state::{TraceEvent, TraceKind};

impl RepoPickerState {
    #[cfg(test)]
    pub(in crate::interfaces::tui) fn enter_context(
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
        self.reset_inputs_and_pending_changes();
        self.status = RepoStatus::Ready;
    }

    pub(in crate::interfaces::tui) fn enter_loading(
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
        self.reset_inputs_and_pending_changes();
        self.logs.clear();
        self.status = RepoStatus::Loading {
            message: "Loading: attached, workspaces, local, GitHub".to_string(),
        };
        self.activity_frame = 0;
        self.push_log(TraceKind::Run, "repository context load started");
    }

    pub(in crate::interfaces::tui) fn start_loading(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status = RepoStatus::Loading {
            message: message.clone(),
        };
        self.activity_frame = 0;
        self.push_log(TraceKind::Run, message);
    }

    pub(in crate::interfaces::tui) fn update_loading_message(
        &mut self,
        message: impl Into<String>,
    ) {
        if let RepoStatus::Loading { message: current } = &mut self.status {
            *current = message.into();
        }
    }

    pub(in crate::interfaces::tui) fn finish_loading(&mut self) {
        if matches!(self.status, RepoStatus::Loading { .. }) {
            self.status = RepoStatus::Ready;
        }
    }

    pub(in crate::interfaces::tui) fn update_attached(
        &mut self,
        attached: Vec<AttachedRepository>,
    ) {
        self.attached = attached;
        self.clear_pending_changes();
        self.status = RepoStatus::Ready;
        self.clamp_selection();
    }

    pub(in crate::interfaces::tui) fn update_attached_from_load(
        &mut self,
        attached: Vec<AttachedRepository>,
    ) {
        self.attached = attached;
        self.clear_pending_changes();
        self.clamp_selection();
    }

    pub(in crate::interfaces::tui) fn update_available_from_load(
        &mut self,
        available: Vec<AvailableRepository>,
    ) {
        self.available = available;
        self.clamp_selection();
    }

    pub(in crate::interfaces::tui) fn update_workspaces_from_load(
        &mut self,
        workspaces: Vec<RepositoryWorkspace>,
    ) {
        let was_empty = self.workspaces.is_empty();
        self.workspaces = workspaces;
        self.clamp_selection();
        if was_empty && !self.workspaces.is_empty() && self.focus == RepoPane::AddPath {
            self.focus = RepoPane::Catalog;
        } else if self.workspaces.is_empty() && self.focus == RepoPane::ConfiguredPaths {
            self.focus = RepoPane::AddPath;
        }
    }

    pub(in crate::interfaces::tui) fn update_candidates_from_load(
        &mut self,
        candidates: Vec<RepositoryCandidate>,
    ) {
        self.candidates = candidates;
        self.pending_link.clear();
        self.clamp_selection();
    }

    pub(in crate::interfaces::tui) fn set_failed(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status = RepoStatus::Failed {
            message: message.clone(),
        };
        self.push_log(TraceKind::Err, message);
    }

    pub(in crate::interfaces::tui) fn start_step(
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

    pub(in crate::interfaces::tui) fn push_log(
        &mut self,
        kind: TraceKind,
        message: impl Into<String>,
    ) {
        self.logs.push(TraceEvent {
            kind,
            message: message.into(),
        });
        if self.logs.len() > 7 {
            self.logs.remove(0);
        }
    }

    pub(in crate::interfaces::tui) fn advance_activity_frame(&mut self) {
        if self.is_activity_active() {
            self.activity_frame = self.activity_frame.wrapping_add(1);
        }
    }

    pub(in crate::interfaces::tui) fn is_activity_active(&self) -> bool {
        matches!(
            self.status,
            RepoStatus::Loading { .. } | RepoStatus::Applying { .. }
        )
    }

    pub(in crate::interfaces::tui::repo_state) fn clear_pending_changes(&mut self) {
        self.pending_add.clear();
        self.pending_link.clear();
        self.pending_remove.clear();
        self.force_remove = false;
    }

    fn reset_inputs_and_pending_changes(&mut self) {
        self.workspace_input.clear();
        self.filter.clear();
        self.clear_pending_changes();
    }
}
