use crate::application::CommandOutput;
use crate::infrastructure::storage::WorkStore;
use crate::shared::error::Result;

pub(crate) fn execute(store: &WorkStore, query: &str) -> Result<CommandOutput> {
    Ok(CommandOutput::WorkOpened(store.open(query)?))
}
