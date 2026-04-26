use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::domain::{
    AttachedRepository, AvailableRepository, RepositoryCandidate, RepositoryWorkspace,
};

use super::state::TraceEvent;

mod actions;
mod catalog;
mod lifecycle;
mod navigation;
mod selection;
mod status;

#[cfg(test)]
mod tests;

pub(super) use actions::RepoPickerAction;
pub(super) use catalog::RepoCatalogRow;
pub(super) use status::{RepoOperation, RepoStatus};

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
    pub(super) fn requires_workspace_setup(&self) -> bool {
        self.workspaces.is_empty() && !matches!(self.status, RepoStatus::Loading { .. })
    }

    pub(super) fn workspace_input(&self) -> &str {
        &self.workspace_input
    }

    pub(super) fn selected_workspace_path(&self) -> Option<PathBuf> {
        self.workspaces
            .get(self.selected_workspace)
            .map(|workspace| workspace.path.clone())
    }
}
