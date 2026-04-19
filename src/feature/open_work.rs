use crate::app::CommandOutput;
use crate::error::Result;
use crate::storage::WorkStore;

pub(crate) fn execute(store: &WorkStore, query: &str) -> Result<CommandOutput> {
    Ok(CommandOutput::WorkOpened(store.open(query)?))
}
