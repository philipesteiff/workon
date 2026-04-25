mod agent_files;
mod app;
mod cli;
mod cli_args;
mod cli_output;
mod domain;
mod error;
mod feature;
mod intents;
mod shell_integration;
mod slug;
mod storage;
mod tui;

pub use app::{App, Command, CommandOutput};
pub use cli::run_cli;
pub use domain::{
    AmbiguousWorkMatch, ArchivedWork, AttachedRepository, AvailableRepository, ContextStatus,
    CreatedWork, IntentProfile, OpenedWork, RepositoryCatalog, RepositoryContextChange, WorkList,
    WorkRepositoryList, WorkSummary,
};
pub use error::{Result, WorkonError};
