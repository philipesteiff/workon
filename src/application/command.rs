use std::path::PathBuf;

use crate::domain::{
    ArchivedWork, ContextStatus, CreatedWork, IntentList, IntentProfile, IntentProfileChange,
    OpenedWork, RepositoryCandidateList, RepositoryCatalog, RepositoryContextChange,
    RepositoryWorkspaceList, WorkIntentSwitch, WorkList, WorkRepositoryList,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    ArchiveWork {
        query: String,
    },
    Context,
    ArchiveIntent {
        intent_id: String,
    },
    CreateIntent {
        input: IntentProfileInput,
    },
    DuplicateIntent {
        source_intent_id: String,
        new_intent_id: String,
        name: Option<String>,
    },
    EditIntent {
        intent_id: String,
        patch: IntentProfilePatch,
    },
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
    ListIntents,
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
    ShowIntent {
        intent_id: String,
    },
    SwitchWorkIntent {
        query: String,
        intent_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutput {
    WorkArchived(ArchivedWork),
    Context(ContextStatus),
    IntentArchived(IntentProfileChange),
    IntentCreated(IntentProfileChange),
    IntentDuplicated(IntentProfileChange),
    IntentList(IntentList),
    IntentShown(IntentProfileChange),
    IntentUpdated(IntentProfileChange),
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
    WorkIntentSwitched(WorkIntentSwitch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentProfileInput {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub skill_weights: Vec<String>,
    pub mcp_weights: Vec<String>,
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntentProfilePatch {
    pub name: Option<String>,
    pub summary: Option<String>,
    pub skill_weights: Option<Vec<String>>,
    pub mcp_weights: Option<Vec<String>>,
    pub instructions: Option<Vec<String>>,
}

impl From<IntentProfileInput> for IntentProfile {
    fn from(input: IntentProfileInput) -> Self {
        Self {
            id: input.id,
            name: input.name,
            summary: input.summary,
            skill_weights: input.skill_weights,
            mcp_weights: input.mcp_weights,
            instructions: input.instructions,
        }
    }
}
