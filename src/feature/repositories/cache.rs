use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::AvailableRepository;
use crate::error::{Result, WorkonError};

use super::github::GithubClient;
use super::paths::repository_cache_path;

pub(super) trait RepositoryCache {
    fn ensure(&self, repository: &AvailableRepository) -> Result<PathBuf>;
    fn path_for(&self, name_with_owner: &str) -> Result<PathBuf>;
}

pub(super) struct BareRepositoryCache<'a> {
    root: &'a Path,
    github: &'a dyn GithubClient,
}

impl<'a> BareRepositoryCache<'a> {
    pub(super) fn new(root: &'a Path, github: &'a dyn GithubClient) -> Self {
        Self { root, github }
    }
}

impl RepositoryCache for BareRepositoryCache<'_> {
    fn ensure(&self, repository: &AvailableRepository) -> Result<PathBuf> {
        let cache_path = self.path_for(&repository.name_with_owner)?;
        if cache_path.exists() {
            self.github.fetch_cache(&cache_path)?;
            return Ok(cache_path);
        }

        fs::create_dir_all(
            cache_path
                .parent()
                .ok_or_else(|| WorkonError::RepositoryContext {
                    message: format!(
                        "repository cache path has no parent: {}",
                        cache_path.display()
                    ),
                })?,
        )?;
        self.github
            .clone_bare(&repository.name_with_owner, &cache_path)?;
        Ok(cache_path)
    }

    fn path_for(&self, name_with_owner: &str) -> Result<PathBuf> {
        repository_cache_path(self.root, name_with_owner)
    }
}
