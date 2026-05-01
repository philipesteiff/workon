mod application;
mod domain;
mod infrastructure;
mod interfaces;
mod shared;

pub use application::{App, Command, CommandOutput};
pub use domain::{
    AmbiguousWorkMatch, ArchivedWork, AttachedRepository, AvailableRepository, CreatedWork,
    IntentList, IntentProfile, IntentProfileChange, IntentSource, IntentSummary, OpenedWork,
    RepositoryAttachment, RepositoryCatalog, RepositoryContextChange, WorkIntentSwitch, WorkList,
    WorkRepositoryList, WorkSummary,
};
pub use interfaces::cli::run_cli;
pub use shared::error::{Result, WorkonError};
