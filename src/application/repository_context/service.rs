use crate::application::CommandOutput;
use std::path::{Path, PathBuf};

use crate::domain::repository_context::paths::{
    normalize_requested_repositories, repository_alias, work_branch, workspace_worktree_path,
};
use crate::domain::{
    RepositoryAttachment, RepositoryCatalog, RepositoryContextChange, WorkRepositoryList,
    WorkSummary,
};
use crate::infrastructure::agent_files::RepoContextFileWriter;
use crate::infrastructure::git_worktree::{RepositoryLinker, WorktreeInspector, WorktreeManager};
use crate::infrastructure::github::GithubClient;
use crate::infrastructure::storage::{
    JsonRepositoryCandidateCache, RepoMetadataStore, RepoWorkspaceStore, RepositoryCache, WorkStore,
};
use crate::shared::error::{Result, WorkonError};

use super::{attachments::RepositoryContextEditor, paths, workspace};

pub(super) struct RepositoryContextService<'a> {
    store: &'a WorkStore,
    github: &'a dyn GithubClient,
    cache: &'a dyn RepositoryCache,
    worktrees: &'a dyn WorktreeManager,
    inspector: &'a dyn WorktreeInspector,
    linker: &'a dyn RepositoryLinker,
    metadata: &'a dyn RepoMetadataStore,
    workspaces: &'a dyn RepoWorkspaceStore,
    context_files: &'a dyn RepoContextFileWriter,
}

pub(super) struct RepositoryContextDeps<'a> {
    pub(super) store: &'a WorkStore,
    pub(super) github: &'a dyn GithubClient,
    pub(super) cache: &'a dyn RepositoryCache,
    pub(super) worktrees: &'a dyn WorktreeManager,
    pub(super) inspector: &'a dyn WorktreeInspector,
    pub(super) linker: &'a dyn RepositoryLinker,
    pub(super) metadata: &'a dyn RepoMetadataStore,
    pub(super) workspaces: &'a dyn RepoWorkspaceStore,
    pub(super) context_files: &'a dyn RepoContextFileWriter,
}

impl<'a> RepositoryContextService<'a> {
    pub(super) fn new(deps: RepositoryContextDeps<'a>) -> Self {
        Self {
            store: deps.store,
            github: deps.github,
            cache: deps.cache,
            worktrees: deps.worktrees,
            inspector: deps.inspector,
            linker: deps.linker,
            metadata: deps.metadata,
            workspaces: deps.workspaces,
            context_files: deps.context_files,
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
        inspector: &dyn WorktreeInspector,
        query: &str,
    ) -> Result<CommandOutput> {
        let work = store.open(query)?;
        let metadata = metadata.read(&work.path)?;
        let repositories = inspector.scan(&work.path, &metadata)?;
        Ok(CommandOutput::WorkRepositories(WorkRepositoryList {
            work: work.into(),
            repositories,
        }))
    }

    pub(super) fn workspaces(workspaces: &dyn RepoWorkspaceStore) -> Result<CommandOutput> {
        workspace::list(workspaces)
    }

    pub(super) fn add_workspaces(
        workspaces: &dyn RepoWorkspaceStore,
        paths: &[PathBuf],
    ) -> Result<CommandOutput> {
        workspace::add(workspaces, paths)
    }

    pub(super) fn remove_workspace(
        store: &WorkStore,
        workspaces: &dyn RepoWorkspaceStore,
        metadata: &dyn RepoMetadataStore,
        path: &Path,
    ) -> Result<CommandOutput> {
        workspace::remove(store, workspaces, metadata, path)
    }

    pub(super) fn discover(
        store: &WorkStore,
        workspaces: &dyn RepoWorkspaceStore,
        inspector: &dyn WorktreeInspector,
        query: &str,
    ) -> Result<CommandOutput> {
        workspace::discover(store, workspaces, inspector, query)
    }

    pub(super) fn candidate_paths(
        store: &WorkStore,
        workspaces: &dyn RepoWorkspaceStore,
        inspector: &dyn WorktreeInspector,
        cache: &JsonRepositoryCandidateCache,
        query: &str,
    ) -> Result<CommandOutput> {
        workspace::candidate_paths(store, workspaces, inspector, cache, query)
    }

    pub(super) fn inspect_candidate(
        inspector: &dyn WorktreeInspector,
        cache: &JsonRepositoryCandidateCache,
        path: &Path,
        refresh: bool,
    ) -> Result<CommandOutput> {
        workspace::inspect_candidate(inspector, cache, path, refresh)
    }

    pub(super) fn add(
        &self,
        query: &str,
        repositories: &[String],
        workspace: Option<&Path>,
    ) -> Result<CommandOutput> {
        let work = self.store.open(query)?;
        let work_summary = WorkSummary::from(work.clone());
        let requested = normalize_requested_repositories(repositories)?;
        let mut editor = self.editor_for(work_summary)?;
        let mut changed = Vec::new();
        let workspace = workspace::select(self.workspaces, workspace)?;

        for name_with_owner in requested {
            if editor.is_attached(&name_with_owner) {
                continue;
            }

            let available = self.github.fetch_repository(&name_with_owner)?;
            let cache_path = self.cache.ensure(&available)?;
            let branch = work_branch(&work);
            let target_path =
                workspace_worktree_path(&workspace.path, &work.slug, &available.name_with_owner)?;

            self.worktrees.switch(
                &cache_path,
                &target_path,
                &branch,
                &available.default_branch,
            )?;
            let alias = repository_alias(&available.name_with_owner)?;
            let link_path =
                self.linker
                    .link(&work.path, &alias, &available.name_with_owner, &target_path)?;

            let attachment = RepositoryAttachment {
                name_with_owner: available.name_with_owner,
                default_branch: available.default_branch,
                url: available.url,
                alias: paths::link_alias(&link_path)?,
                target_path: paths::canonicalize(&target_path)?,
            };
            changed.push(editor.add(
                attachment,
                "repository worktree was created but could not be inspected",
            )?);
        }

        Ok(CommandOutput::WorkRepositoriesAdded(
            RepositoryContextChange {
                work: work.into(),
                repositories: changed,
            },
        ))
    }

    pub(super) fn link(&self, query: &str, paths: &[PathBuf]) -> Result<CommandOutput> {
        let work = self.store.open(query)?;
        let work_summary = WorkSummary::from(work.clone());
        let mut editor = self.editor_for(work_summary)?;
        let mut changed = Vec::new();

        for path in paths {
            let target_path = paths::canonicalize(path)?;
            let Some(candidate) = self.inspector.inspect_candidate(&target_path)? else {
                return Err(WorkonError::RepositoryContext {
                    message: format!("not a Git working tree: {}", path.display()),
                });
            };
            if editor.is_attached(&candidate.name_with_owner) {
                continue;
            }

            let alias = repository_alias(&candidate.name_with_owner)?;
            let link_path =
                self.linker
                    .link(&work.path, &alias, &candidate.name_with_owner, &target_path)?;
            let attachment = RepositoryAttachment {
                name_with_owner: candidate.name_with_owner,
                default_branch: String::new(),
                url: candidate.url,
                alias: paths::link_alias(&link_path)?,
                target_path,
            };
            changed.push(editor.add(
                attachment,
                "repository was linked but could not be inspected",
            )?);
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
        _force: bool,
    ) -> Result<CommandOutput> {
        let work = self.store.open(query)?;
        let work_summary = WorkSummary::from(work.clone());
        let requested = normalize_requested_repositories(repositories)?;
        let mut editor = self.editor_for(work_summary)?;
        let mut removed = Vec::new();

        for name_with_owner in &requested {
            removed.push(editor.remove(name_with_owner, self.linker)?);
        }

        Ok(CommandOutput::WorkRepositoriesRemoved(
            RepositoryContextChange {
                work: work.into(),
                repositories: removed,
            },
        ))
    }

    fn editor_for(&self, work: WorkSummary) -> Result<RepositoryContextEditor<'_>> {
        let metadata = self.metadata.read(&work.path)?;
        let attached = self.inspector.scan(&work.path, &metadata)?;
        Ok(RepositoryContextEditor::new(
            work,
            metadata,
            attached,
            self.inspector,
            self.metadata,
            self.context_files,
        ))
    }
}
