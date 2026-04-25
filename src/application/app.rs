use std::path::PathBuf;

use crate::application::command::{Command, CommandOutput};
use crate::application::{context_status, repository_context, shell, work};
use crate::domain::IntentCatalog;
use crate::infrastructure::storage::WorkStore;
use crate::shared::error::Result;

#[derive(Debug, Clone)]
pub struct App {
    store: WorkStore,
    intents: IntentCatalog,
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        Self {
            store: WorkStore::new(root),
            intents: IntentCatalog::default_catalog(),
        }
    }

    pub fn execute(&self, command: Command) -> Result<CommandOutput> {
        match command {
            Command::AddWorkRepositories {
                query,
                repositories,
            } => repository_context::add(&self.store, &self.intents, &query, &repositories),
            Command::ArchiveWork { query } => work::archive::execute(&self.store, &query),
            Command::Context => context_status::execute(),
            Command::CreateWork { goal, intent_id } => {
                work::create::execute(&self.store, &self.intents, &goal, &intent_id)
            }
            Command::InstallDevShell { manifest_path } => {
                shell::install::install_dev_shell(manifest_path)
            }
            Command::InstallShell => shell::install::install_shell(),
            Command::ListGitHubRepositories => repository_context::list_available(),
            Command::ListWorkRepositories { query } => {
                repository_context::list_attached(&self.store, &query)
            }
            Command::ListWorks => work::list::execute(&self.store),
            Command::OpenOrCreate { input, intent_id } => work::open_or_create::execute(
                &self.store,
                &self.intents,
                &input,
                intent_id.as_deref(),
            ),
            Command::OpenWork { query } => work::open::execute(&self.store, &query),
            Command::RemoveWorkRepositories {
                query,
                repositories,
                force,
            } => {
                repository_context::remove(&self.store, &self.intents, &query, &repositories, force)
            }
        }
    }

    pub fn available_intents(&self) -> Vec<(String, String)> {
        self.intents
            .profiles()
            .iter()
            .map(|intent| (intent.id.clone(), intent.summary.clone()))
            .collect()
    }
}
