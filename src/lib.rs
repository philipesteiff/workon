mod application;
mod domain;
mod infrastructure;
mod interfaces;
mod shared;

pub use application::{App, Command, CommandOutput};
pub use domain::{
    AmbiguousWorkMatch, ArchivedWork, AttachedRepository, AvailableRepository, ContextStatus,
    CreatedWork, IntentProfile, OpenedWork, RepositoryCatalog, RepositoryContextChange, WorkList,
    WorkRepositoryList, WorkSummary,
};
pub use interfaces::cli::run_cli;
pub use shared::error::{Result, WorkonError};
