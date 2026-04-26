use std::path::PathBuf;

use crate::application::command::{Command, CommandOutput};
use crate::application::{intent, repository_context, shell, work};
use crate::domain::IntentCatalog;
use crate::infrastructure::storage::{JsonIntentStore, WorkStore};
use crate::shared::error::Result;

#[derive(Debug, Clone)]
pub struct App {
    store: WorkStore,
    intent_store: JsonIntentStore,
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        Self {
            store: WorkStore::new(root.clone()),
            intent_store: JsonIntentStore::new(&root),
        }
    }

    pub fn execute(&self, command: Command) -> Result<CommandOutput> {
        match command {
            Command::AddWorkRepositories {
                query,
                repositories,
                workspace,
            } => {
                let intents = self.intent_catalog()?;
                repository_context::add(
                    &self.store,
                    &intents,
                    &query,
                    &repositories,
                    workspace.as_deref(),
                )
            }
            Command::AddRepositoryWorkspaces { paths } => {
                repository_context::add_workspaces(&self.store, &paths)
            }
            Command::ArchiveIntent { intent_id } => {
                let intents = self.intent_catalog()?;
                intent::archive(&self.intent_store, &intents, &intent_id)
            }
            Command::ArchiveWork { query } => work::archive::execute(&self.store, &query),
            Command::CreateIntent { input } => {
                let intents = self.intent_catalog()?;
                intent::create(&self.intent_store, &intents, input)
            }
            Command::CreateWork { goal, intent_id } => {
                let intents = self.intent_catalog()?;
                work::create::execute(&self.store, &intents, &goal, &intent_id)
            }
            Command::DuplicateIntent {
                source_intent_id,
                new_intent_id,
                name,
            } => {
                let intents = self.intent_catalog()?;
                intent::duplicate(
                    &self.intent_store,
                    &intents,
                    &source_intent_id,
                    &new_intent_id,
                    name,
                )
            }
            Command::EditIntent { intent_id, patch } => {
                let intents = self.intent_catalog()?;
                intent::edit(&self.intent_store, &intents, &intent_id, patch)
            }
            Command::InstallDevShell { manifest_path } => {
                shell::install::install_dev_shell(manifest_path)
            }
            Command::InstallShell => shell::install::install_shell(),
            Command::ListIntents => {
                let intents = self.intent_catalog()?;
                intent::list(&intents)
            }
            Command::ListGitHubRepositories => repository_context::list_available(),
            Command::ListRepositoryCandidates { query } => {
                repository_context::discover(&self.store, &query)
            }
            Command::ListRepositoryWorkspaces => repository_context::list_workspaces(&self.store),
            Command::ListWorkRepositories { query } => {
                repository_context::list_attached(&self.store, &query)
            }
            Command::ListWorks => work::list::execute(&self.store),
            Command::OpenOrCreate { input, intent_id } => {
                let intents = self.intent_catalog()?;
                work::open_or_create::execute(&self.store, &intents, &input, intent_id.as_deref())
            }
            Command::OpenWork { query } => work::open::execute(&self.store, &query),
            Command::LinkWorkRepositories { query, paths } => {
                let intents = self.intent_catalog()?;
                repository_context::link(&self.store, &intents, &query, &paths)
            }
            Command::RemoveRepositoryWorkspace { path } => {
                repository_context::remove_workspace(&self.store, &path)
            }
            Command::RemoveWorkRepositories {
                query,
                repositories,
                force,
            } => {
                let intents = self.intent_catalog()?;
                repository_context::remove(&self.store, &intents, &query, &repositories, force)
            }
            Command::ShowIntent { intent_id } => {
                let intents = self.intent_catalog()?;
                intent::show(&intents, &intent_id)
            }
            Command::SwitchWorkIntent { query, intent_id } => {
                let intents = self.intent_catalog()?;
                intent::switch_work_intent(&self.store, &intents, &query, &intent_id)
            }
        }
    }

    pub fn available_intents(&self) -> Vec<(String, String)> {
        self.intent_catalog()
            .unwrap_or_else(|_| IntentCatalog::default_catalog())
            .profiles()
            .iter()
            .map(|intent| (intent.id.clone(), intent.summary.clone()))
            .collect()
    }

    fn intent_catalog(&self) -> Result<IntentCatalog> {
        Ok(IntentCatalog::with_custom(
            self.intent_store.active_profiles()?,
        ))
    }
}
