pub mod agent_context;
pub mod intent;
pub mod repository_context;
pub mod work;

pub use agent_context::ContextStatus;
pub use intent::{IntentCatalog, IntentProfile};
pub use repository_context::{
    AttachedRepository, AvailableRepository, RepositoryAttachment, RepositoryCatalog,
    RepositoryContextChange, WorkRepositoryList,
};
pub use work::{AmbiguousWorkMatch, ArchivedWork, CreatedWork, OpenedWork, WorkList, WorkSummary};
