mod git;
mod inspector;
mod linker;

use std::path::{Path, PathBuf};

use crate::domain::{AttachedRepository, RepositoryAttachment, RepositoryCandidate};
use crate::shared::error::Result;

pub(crate) use git::GitWorktree;
pub(crate) use linker::SymlinkRepositoryLinker;

pub(crate) trait WorktreeManager {
    fn switch(
        &self,
        cache_path: &Path,
        worktree_path: &Path,
        branch: &str,
        default_branch: &str,
    ) -> Result<()>;
}

pub(crate) trait WorktreeInspector {
    fn scan(
        &self,
        work_path: &Path,
        metadata: &[RepositoryAttachment],
    ) -> Result<Vec<AttachedRepository>>;
    fn inspect_candidate(&self, path: &Path) -> Result<Option<RepositoryCandidate>>;
    fn discover(&self, roots: &[PathBuf]) -> Result<Vec<RepositoryCandidate>>;
}

pub(crate) trait RepositoryLinker {
    fn link(
        &self,
        work_path: &Path,
        preferred_alias: &str,
        name_with_owner: &str,
        target_path: &Path,
    ) -> Result<PathBuf>;
    fn remove(&self, link_path: &Path) -> Result<()>;
}

#[cfg(test)]
mod tests;
