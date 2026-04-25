use std::path::PathBuf;

use crate::domain::{
    ArchivedWork, ContextStatus, CreatedWork, OpenedWork, RepositoryCatalog,
    RepositoryContextChange, WorkList, WorkRepositoryList,
};
use crate::error::Result;
use crate::feature;
use crate::intents::IntentCatalog;
use crate::storage::WorkStore;

#[derive(Debug, Clone)]
pub struct App {
    store: WorkStore,
    intents: IntentCatalog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    ArchiveWork {
        query: String,
    },
    Context,
    AddWorkRepositories {
        query: String,
        repositories: Vec<String>,
    },
    CreateWork {
        goal: String,
        intent_id: String,
    },
    InstallDevShell {
        manifest_path: PathBuf,
    },
    InstallShell,
    ListGitHubRepositories,
    ListWorkRepositories {
        query: String,
    },
    ListWorks,
    OpenOrCreate {
        input: String,
        intent_id: Option<String>,
    },
    OpenWork {
        query: String,
    },
    RemoveWorkRepositories {
        query: String,
        repositories: Vec<String>,
        force: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutput {
    WorkArchived(ArchivedWork),
    Context(ContextStatus),
    RepositoryCatalog(RepositoryCatalog),
    ShellInstalled {
        label: String,
        script_path: PathBuf,
        zshrc_path: PathBuf,
    },
    WorkCreated(CreatedWork),
    WorkList(WorkList),
    WorkOpened(OpenedWork),
    WorkRepositories(WorkRepositoryList),
    WorkRepositoriesAdded(RepositoryContextChange),
    WorkRepositoriesRemoved(RepositoryContextChange),
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
            } => feature::repositories::add(&self.store, &self.intents, &query, &repositories),
            Command::ArchiveWork { query } => feature::archive_work::execute(&self.store, &query),
            Command::Context => feature::context::execute(),
            Command::CreateWork { goal, intent_id } => {
                feature::create_work::execute(&self.store, &self.intents, &goal, &intent_id)
            }
            Command::InstallDevShell { manifest_path } => {
                feature::install_shell::install_dev_shell(manifest_path)
            }
            Command::InstallShell => feature::install_shell::install_shell(),
            Command::ListGitHubRepositories => feature::repositories::list_available(),
            Command::ListWorkRepositories { query } => {
                feature::repositories::list_attached(&self.store, &query)
            }
            Command::ListWorks => feature::list_works::execute(&self.store),
            Command::OpenOrCreate { input, intent_id } => feature::open_or_create::execute(
                &self.store,
                &self.intents,
                &input,
                intent_id.as_deref(),
            ),
            Command::OpenWork { query } => feature::open_work::execute(&self.store, &query),
            Command::RemoveWorkRepositories {
                query,
                repositories,
                force,
            } => feature::repositories::remove(
                &self.store,
                &self.intents,
                &query,
                &repositories,
                force,
            ),
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
