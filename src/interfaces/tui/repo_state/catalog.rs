use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::domain::{AttachedRepository, AvailableRepository, RepositoryCandidate};

use super::RepoPickerState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::interfaces::tui) enum RepoCatalogRow {
    GitHub(AvailableRepository),
    Local(RepositoryCandidate),
    Attached(AttachedRepository),
}

impl RepoCatalogRow {
    pub(in crate::interfaces::tui) fn name(&self) -> &str {
        match self {
            Self::GitHub(repository) => &repository.name_with_owner,
            Self::Local(candidate) => &candidate.name_with_owner,
            Self::Attached(repository) => &repository.name_with_owner,
        }
    }

    pub(in crate::interfaces::tui) fn meta(&self) -> String {
        match self {
            Self::GitHub(repository) => format!("github default {}", repository.default_branch),
            Self::Local(candidate) => {
                format!("local {} {}", candidate.branch, candidate.path.display())
            }
            Self::Attached(repository) => format!("attached {}", repository.branch),
        }
    }
}

impl RepoPickerState {
    pub(in crate::interfaces::tui) fn catalog_rows(&self) -> Vec<RepoCatalogRow> {
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
            let left_rank = catalog_row_rank(left, &attached_names);
            let right_rank = catalog_row_rank(right, &attached_names);
            left_rank.cmp(&right_rank).then(
                left.name()
                    .cmp(right.name())
                    .then(left.meta().cmp(&right.meta())),
            )
        });
        rows
    }

    pub(in crate::interfaces::tui) fn is_selected(&self, name_with_owner: &str) -> bool {
        if self.pending_remove.contains(name_with_owner) {
            return false;
        }
        self.pending_add.contains(name_with_owner)
            || self
                .attached
                .iter()
                .any(|repository| repository.name_with_owner == name_with_owner)
    }

    pub(in crate::interfaces::tui) fn is_local_candidate_selected(&self, path: &PathBuf) -> bool {
        self.pending_link.contains(path)
    }
}

fn catalog_row_rank(row: &RepoCatalogRow, attached_names: &BTreeSet<String>) -> u8 {
    if attached_names.contains(row.name()) {
        return 0;
    }
    match row {
        RepoCatalogRow::Local(_) => 1,
        RepoCatalogRow::GitHub(_) | RepoCatalogRow::Attached(_) => 2,
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
