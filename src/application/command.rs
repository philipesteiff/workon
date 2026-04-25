use std::path::PathBuf;

use crate::domain::{
    ArchivedWork, ContextStatus, CreatedWork, OpenedWork, RepositoryCatalog,
    RepositoryContextChange, WorkList, WorkRepositoryList,
};

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
