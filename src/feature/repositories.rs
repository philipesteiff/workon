mod cache;
mod context_files;
mod github;
mod metadata;
mod paths;
mod process;
mod service;
mod worktree;

use crate::app::CommandOutput;
use crate::error::Result;
use crate::intents::IntentCatalog;
use crate::storage::WorkStore;

use self::cache::BareRepositoryCache;
use self::context_files::AgentRepoContextFileWriter;
use self::github::GhCli;
use self::metadata::JsonRepoMetadataStore;
use self::process::StdProcessRunner;
use self::service::RepositoryContextService;
use self::worktree::Worktrunk;

pub(crate) fn list_available() -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let github = GhCli::new(&runner);
    RepositoryContextService::catalog(&github)
}

pub(crate) fn list_attached(store: &WorkStore, query: &str) -> Result<CommandOutput> {
    let metadata = JsonRepoMetadataStore;
    RepositoryContextService::attached(store, &metadata, query)
}

pub(crate) fn add(
    store: &WorkStore,
    intents: &IntentCatalog,
    query: &str,
    repositories: &[String],
) -> Result<CommandOutput> {
    let runner = StdProcessRunner;
    let github = GhCli::new(&runner);
    let cache = BareRepositoryCache::new(store.root(), &github);
    let worktrees = Worktrunk::new(&runner);
    let metadata = JsonRepoMetadataStore;
    let context_files = AgentRepoContextFileWriter::new(intents);
    let service = RepositoryContextService::new(
        store,
        &github,
        &cache,
        &worktrees,
        &metadata,
        &context_files,
    );

    service.add(query, repositories)
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
    let worktrees = Worktrunk::new(&runner);
    let metadata = JsonRepoMetadataStore;
    let context_files = AgentRepoContextFileWriter::new(intents);
    let service = RepositoryContextService::new(
        store,
        &github,
        &cache,
        &worktrees,
        &metadata,
        &context_files,
    );

    service.remove(query, repositories, force)
}
