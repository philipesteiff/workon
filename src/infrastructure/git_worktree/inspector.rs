use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::repository_context::paths::repository_name_from_remote_url;
use crate::domain::{AttachedRepository, RepositoryAttachment, RepositoryCandidate};
use crate::shared::error::Result;

use super::git::GitWorktree;
use super::linker::symlink_target;
use super::WorktreeInspector;

impl GitWorktree<'_> {
    fn inspected_repository(
        &self,
        path: &Path,
        metadata: Option<&RepositoryAttachment>,
    ) -> Result<Option<AttachedRepository>> {
        let Some(branch) = self.scan_branch(path) else {
            return Ok(None);
        };
        let url = self
            .origin_url(path)
            .or_else(|| metadata.map(|repository| repository.url.clone()))
            .unwrap_or_default();
        let name_with_owner = repository_name_from_remote_url(&url)
            .or_else(|| metadata.map(|repository| repository.name_with_owner.clone()));
        let Some(name_with_owner) = name_with_owner else {
            return Ok(None);
        };
        let default_branch = metadata
            .map(|repository| repository.default_branch.clone())
            .unwrap_or_default();

        Ok(Some(AttachedRepository {
            name_with_owner,
            branch,
            path: path.to_path_buf(),
            default_branch,
            url,
        }))
    }

    fn candidate_at(&self, path: &Path) -> Result<Option<RepositoryCandidate>> {
        let Some(repository) = self.inspected_repository(path, None)? else {
            return Ok(None);
        };
        Ok(Some(RepositoryCandidate {
            name_with_owner: repository.name_with_owner,
            branch: repository.branch,
            path: path.to_path_buf(),
            url: repository.url,
        }))
    }
}

impl WorktreeInspector for GitWorktree<'_> {
    fn scan(
        &self,
        work_path: &Path,
        metadata: &[RepositoryAttachment],
    ) -> Result<Vec<AttachedRepository>> {
        let repos_path = work_path.join("repos");
        if !repos_path.exists() {
            return Ok(Vec::new());
        }

        let metadata_by_alias = metadata
            .iter()
            .filter(|repository| !repository.alias.is_empty())
            .map(|repository| (repository.alias.as_str(), repository))
            .collect::<BTreeMap<_, _>>();
        let metadata_by_target = metadata
            .iter()
            .filter(|repository| !repository.target_path.as_os_str().is_empty())
            .map(|repository| (repository.target_path.as_path(), repository))
            .collect::<BTreeMap<_, _>>();
        let mut repositories = Vec::new();

        for entry in fs::read_dir(repos_path)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = fs::symlink_metadata(&path)?.file_type();
            let is_symlink = file_type.is_symlink();
            if !path.is_dir() && !is_symlink {
                continue;
            }

            let folder_name = entry.file_name();
            let alias = folder_name.to_string_lossy();
            let target = fs::canonicalize(&path)
                .or_else(|_| symlink_target(&path))
                .unwrap_or_else(|_| path.clone());
            let metadata = metadata_by_alias
                .get(alias.as_ref())
                .copied()
                .or_else(|| metadata_by_target.get(target.as_path()).copied());
            if let Some(repository) = self.inspected_repository(&path, metadata)? {
                repositories.push(repository);
            } else if is_symlink && !path.exists() {
                if let Some(metadata) = metadata {
                    repositories.push(missing_repository_link(&path, metadata));
                }
            }
        }

        repositories.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
        Ok(repositories)
    }

    fn inspect_candidate(&self, path: &Path) -> Result<Option<RepositoryCandidate>> {
        self.candidate_at(path)
    }

    fn discover(&self, roots: &[PathBuf]) -> Result<Vec<RepositoryCandidate>> {
        let mut candidates = Vec::new();
        for root in roots {
            discover_root(self, root, 0, &mut candidates)?;
        }
        candidates.sort_by(|left, right| {
            left.name_with_owner
                .cmp(&right.name_with_owner)
                .then(left.path.cmp(&right.path))
        });
        candidates.dedup_by(|left, right| left.path == right.path);
        Ok(candidates)
    }
}

fn discover_root(
    git: &GitWorktree<'_>,
    path: &Path,
    depth: usize,
    candidates: &mut Vec<RepositoryCandidate>,
) -> Result<()> {
    const MAX_DEPTH: usize = 4;
    if depth > MAX_DEPTH || !path.exists() || ignored_discovery_path(path) {
        return Ok(());
    }

    if let Some(candidate) = git.candidate_at(path)? {
        candidates.push(candidate);
        return Ok(());
    }

    if !path.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            discover_root(git, &child, depth + 1, candidates)?;
        }
    }
    Ok(())
}

fn ignored_discovery_path(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | "node_modules" | "target" | ".cache" | ".workon")
    )
}

fn missing_repository_link(path: &Path, metadata: &RepositoryAttachment) -> AttachedRepository {
    AttachedRepository {
        name_with_owner: metadata.name_with_owner.clone(),
        branch: "missing".to_string(),
        path: path.to_path_buf(),
        default_branch: metadata.default_branch.clone(),
        url: metadata.url.clone(),
    }
}
