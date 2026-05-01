use crate::domain::RepositoryCandidate;

use super::actions::RepoPickerAction;
use super::catalog::RepoCatalogRow;
use super::{RepoPane, RepoPickerState};
use crate::interfaces::tui::state::{Toast, TraceKind};

impl RepoPickerState {
    pub(in crate::interfaces::tui::repo_state) fn toggle_focused_repository(&mut self) {
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
                    RepoCatalogRow::PendingLocal { .. } => {}
                    RepoCatalogRow::Attached(repository) => {
                        self.toggle_repository_selection(&repository.name_with_owner)
                    }
                }
            }
            RepoPane::AddPath | RepoPane::ConfiguredPaths => {}
        }
        self.clamp_selection();
    }

    pub(in crate::interfaces::tui::repo_state) fn toggle_force_remove(
        &mut self,
    ) -> RepoPickerAction {
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
}
