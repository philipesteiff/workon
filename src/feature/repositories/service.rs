use std::fs;

use crate::app::CommandOutput;
use crate::domain::{
    AttachedRepository, RepositoryCatalog, RepositoryContextChange, WorkRepositoryList, WorkSummary,
};
use crate::error::{Result, WorkonError};
use crate::storage::WorkStore;

use super::cache::RepositoryCache;
use super::context_files::RepoContextFileWriter;
use super::github::GithubClient;
use super::metadata::RepoMetadataStore;
use super::paths::{normalize_requested_repositories, work_branch, worktree_path};
use super::worktree::WorktreeManager;

pub(super) struct RepositoryContextService<'a> {
    store: &'a WorkStore,
    github: &'a dyn GithubClient,
    cache: &'a dyn RepositoryCache,
    worktrees: &'a dyn WorktreeManager,
    metadata: &'a dyn RepoMetadataStore,
    context_files: &'a dyn RepoContextFileWriter,
}

impl<'a> RepositoryContextService<'a> {
    pub(super) fn new(
        store: &'a WorkStore,
        github: &'a dyn GithubClient,
        cache: &'a dyn RepositoryCache,
        worktrees: &'a dyn WorktreeManager,
        metadata: &'a dyn RepoMetadataStore,
        context_files: &'a dyn RepoContextFileWriter,
    ) -> Self {
        Self {
            store,
            github,
            cache,
            worktrees,
            metadata,
            context_files,
        }
    }

    pub(super) fn catalog(github: &dyn GithubClient) -> Result<CommandOutput> {
        Ok(CommandOutput::RepositoryCatalog(RepositoryCatalog {
            repositories: github.list_repositories()?,
        }))
    }

    pub(super) fn attached(
        store: &WorkStore,
        metadata: &dyn RepoMetadataStore,
        query: &str,
    ) -> Result<CommandOutput> {
        let work = store.open(query)?;
        let repositories = metadata.read(&work.path)?;
        Ok(CommandOutput::WorkRepositories(WorkRepositoryList {
            work: work.into(),
            repositories,
        }))
    }

    pub(super) fn add(&self, query: &str, repositories: &[String]) -> Result<CommandOutput> {
        let work = self.store.open(query)?;
        let work_summary = WorkSummary::from(work.clone());
        let requested = normalize_requested_repositories(repositories)?;
        let mut attached = self.metadata.read(&work.path)?;
        let mut changed = Vec::new();

        for name_with_owner in requested {
            if let Some(existing) = attached
                .iter()
                .find(|repo| repo.name_with_owner == name_with_owner)
                .cloned()
            {
                changed.push(existing);
                continue;
            }

            let available = self.github.fetch_repository(&name_with_owner)?;
            let cache_path = self.cache.ensure(&available)?;
            let branch = work_branch(&work);
            let path = worktree_path(&work, &available.name_with_owner)?;
            fs::create_dir_all(
                path.parent()
                    .ok_or_else(|| WorkonError::RepositoryContext {
                        message: format!("repository path has no parent: {}", path.display()),
                    })?,
            )?;

            self.worktrees
                .switch(&cache_path, &path, &branch, &available.default_branch)?;

            let repository = AttachedRepository {
                name_with_owner: available.name_with_owner,
                branch,
                path,
                default_branch: available.default_branch,
                url: available.url,
            };
            attached.push(repository.clone());
            attached.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
            self.persist(&work_summary, &attached)?;
            changed.push(repository);
        }

        Ok(CommandOutput::WorkRepositoriesAdded(
            RepositoryContextChange {
                work: work.into(),
                repositories: changed,
            },
        ))
    }

    pub(super) fn remove(
        &self,
        query: &str,
        repositories: &[String],
        force: bool,
    ) -> Result<CommandOutput> {
        let work = self.store.open(query)?;
        let work_summary = WorkSummary::from(work.clone());
        let requested = normalize_requested_repositories(repositories)?;
        let mut attached = self.metadata.read(&work.path)?;
        let mut removed = Vec::new();

        for name_with_owner in &requested {
            let Some(index) = attached
                .iter()
                .position(|repo| &repo.name_with_owner == name_with_owner)
            else {
                continue;
            };
            let repository = attached[index].clone();
            let cache_path = self.cache.path_for(&repository.name_with_owner)?;
            self.worktrees
                .remove(&cache_path, &repository.branch, force)?;
            attached.remove(index);
            self.persist(&work_summary, &attached)?;
            removed.push(repository);
        }

        Ok(CommandOutput::WorkRepositoriesRemoved(
            RepositoryContextChange {
                work: work.into(),
                repositories: removed,
            },
        ))
    }

    fn persist(&self, work: &WorkSummary, repositories: &[AttachedRepository]) -> Result<()> {
        self.metadata.write(&work.path, repositories)?;
        self.context_files.rewrite(work, repositories)
    }
}
