mod service;

use crate::application::CommandOutput;
use crate::domain::IntentCatalog;
use crate::infrastructure::agent_files::AgentRepoContextFileWriter;
use crate::infrastructure::github::GhCli;
use crate::infrastructure::process::StdProcessRunner;
use crate::infrastructure::storage::{BareRepositoryCache, JsonRepoMetadataStore, WorkStore};
use crate::infrastructure::worktrunk::Worktrunk;
use crate::shared::error::Result;

use self::service::RepositoryContextService;

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
