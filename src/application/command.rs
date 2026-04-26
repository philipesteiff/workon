use std::path::PathBuf;

use crate::domain::{
    ArchivedWork, ContextStatus, CreatedWork, OpenedWork, RepositoryCandidateList,
    RepositoryCatalog, RepositoryContextChange, RepositoryWorkspaceList, WorkList,
    WorkRepositoryList,
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
        workspace: Option<PathBuf>,
    },
    AddRepositoryWorkspaces {
        paths: Vec<PathBuf>,
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
    ListRepositoryCandidates {
        query: String,
    },
    ListRepositoryWorkspaces,
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
    LinkWorkRepositories {
        query: String,
        paths: Vec<PathBuf>,
    },
    RemoveRepositoryWorkspace {
        path: PathBuf,
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
    RepositoryCandidates(RepositoryCandidateList),
    RepositoryWorkspaces(RepositoryWorkspaceList),
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
