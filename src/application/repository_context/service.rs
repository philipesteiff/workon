use crate::application::CommandOutput;
use crate::domain::repository_context::paths::{
    normalize_requested_repositories, work_branch, worktree_path,
};
use crate::domain::{
    AttachedRepository, RepositoryCatalog, RepositoryContextChange, WorkRepositoryList, WorkSummary,
};
use crate::infrastructure::agent_files::RepoContextFileWriter;
use crate::infrastructure::git_worktree::WorktreeManager;
use crate::infrastructure::github::GithubClient;
use crate::infrastructure::storage::{RepoMetadataStore, RepositoryCache, WorkStore};
use crate::shared::error::{Result, WorkonError};

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
            if attached
                .iter()
                .any(|repo| repo.name_with_owner == name_with_owner)
            {
                continue;
            }

            let available = self.github.fetch_repository(&name_with_owner)?;
            let cache_path = self.cache.ensure(&available)?;
            let branch = work_branch(&work);
            let path = worktree_path(&work, &available.name_with_owner)?;

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
                return Err(WorkonError::RepositoryContext {
                    message: format!(
                        "repository `{name_with_owner}` is not attached to work `{}`",
                        work.slug
                    ),
                });
            };
            let repository = attached[index].clone();
            let cache_path = self.cache.path_for(&repository.name_with_owner)?;
            self.worktrees
                .remove(&cache_path, &repository.path, force)?;
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
