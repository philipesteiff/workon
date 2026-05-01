mod attachments;
mod paths;
mod service;
#[cfg(test)]
mod service_tests;
mod workspace;

use crate::application::CommandOutput;
use crate::domain::IntentCatalog;
use crate::infrastructure::agent_files::AgentRepoContextFileWriter;
use crate::infrastructure::git_worktree::{GitWorktree, SymlinkRepositoryLinker};
use crate::infrastructure::github::GhCli;
use crate::infrastructure::process::StdProcessRunner;
use crate::infrastructure::storage::{
    BareRepositoryCache, JsonRepoMetadataStore, JsonRepoWorkspaceStore,
    JsonRepositoryCandidateCache, WorkStore,
};
use crate::shared::error::Result;
use std::path::{Path, PathBuf};

use self::service::{RepositoryContextDeps, RepositoryContextService};

pub(crate) fn list_available() -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let github = GhCli::new(&runner);
    RepositoryContextService::catalog(&github)
}

pub(crate) fn list_attached(store: &WorkStore, query: &str) -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let inspector = GitWorktree::new(&runner);
    let metadata = JsonRepoMetadataStore;
    RepositoryContextService::attached(store, &metadata, &inspector, query)
}

pub(crate) fn list_workspaces(store: &WorkStore) -> Result<CommandOutput> {
    let workspaces = JsonRepoWorkspaceStore::new(store.root());
    RepositoryContextService::workspaces(&workspaces)
}

pub(crate) fn add_workspaces(store: &WorkStore, paths: &[PathBuf]) -> Result<CommandOutput> {
    let workspaces = JsonRepoWorkspaceStore::new(store.root());
    RepositoryContextService::add_workspaces(&workspaces, paths)
}

pub(crate) fn remove_workspace(store: &WorkStore, path: &Path) -> Result<CommandOutput> {
    let workspaces = JsonRepoWorkspaceStore::new(store.root());
    let metadata = JsonRepoMetadataStore;
    RepositoryContextService::remove_workspace(store, &workspaces, &metadata, path)
}

pub(crate) fn discover(store: &WorkStore, query: &str) -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let inspector = GitWorktree::new(&runner);
    let workspaces = JsonRepoWorkspaceStore::new(store.root());
    RepositoryContextService::discover(store, &workspaces, &inspector, query)
}

pub(crate) fn candidate_paths(store: &WorkStore, query: &str) -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let inspector = GitWorktree::new(&runner);
    let workspaces = JsonRepoWorkspaceStore::new(store.root());
    let cache = JsonRepositoryCandidateCache::new(store.root());
    RepositoryContextService::candidate_paths(store, &workspaces, &inspector, &cache, query)
}

pub(crate) fn inspect_candidate(
    store: &WorkStore,
    path: &Path,
    refresh: bool,
) -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let inspector = GitWorktree::new(&runner);
    let cache = JsonRepositoryCandidateCache::new(store.root());
    RepositoryContextService::inspect_candidate(&inspector, &cache, path, refresh)
}

pub(crate) fn add(
    store: &WorkStore,
    intents: &IntentCatalog,
    query: &str,
    repositories: &[String],
    workspace: Option<&Path>,
) -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let github = GhCli::new(&runner);
    let cache = BareRepositoryCache::new(store.root(), &github);
    let worktrees = GitWorktree::new(&runner);
    let metadata = JsonRepoMetadataStore;
    let workspaces = JsonRepoWorkspaceStore::new(store.root());
    let linker = SymlinkRepositoryLinker;
    let context_files = AgentRepoContextFileWriter::new(intents);
    let service = RepositoryContextService::new(RepositoryContextDeps {
        store,
        github: &github,
        cache: &cache,
        worktrees: &worktrees,
        inspector: &worktrees,
        linker: &linker,
        metadata: &metadata,
        workspaces: &workspaces,
        context_files: &context_files,
    });

    service.add(query, repositories, workspace)
}

pub(crate) fn link(
    store: &WorkStore,
    intents: &IntentCatalog,
    query: &str,
    paths: &[PathBuf],
) -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let github = GhCli::new(&runner);
    let cache = BareRepositoryCache::new(store.root(), &github);
    let worktrees = GitWorktree::new(&runner);
    let metadata = JsonRepoMetadataStore;
    let workspaces = JsonRepoWorkspaceStore::new(store.root());
    let linker = SymlinkRepositoryLinker;
    let context_files = AgentRepoContextFileWriter::new(intents);
    let service = RepositoryContextService::new(RepositoryContextDeps {
        store,
        github: &github,
        cache: &cache,
        worktrees: &worktrees,
        inspector: &worktrees,
        linker: &linker,
        metadata: &metadata,
        workspaces: &workspaces,
        context_files: &context_files,
    });

    service.link(query, paths)
}

pub(crate) fn remove(
    store: &WorkStore,
    intents: &IntentCatalog,
    query: &str,
    repositories: &[String],
    force: bool,
) -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let github = GhCli::new(&runner);
    let cache = BareRepositoryCache::new(store.root(), &github);
    let worktrees = GitWorktree::new(&runner);
    let metadata = JsonRepoMetadataStore;
    let workspaces = JsonRepoWorkspaceStore::new(store.root());
    let linker = SymlinkRepositoryLinker;
    let context_files = AgentRepoContextFileWriter::new(intents);
    let service = RepositoryContextService::new(RepositoryContextDeps {
        store,
        github: &github,
        cache: &cache,
        worktrees: &worktrees,
        inspector: &worktrees,
        linker: &linker,
        metadata: &metadata,
        workspaces: &workspaces,
        context_files: &context_files,
    });

    service.remove(query, repositories, force)
}
