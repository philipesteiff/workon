use crate::app::CommandOutput;
use crate::error::Result;
use crate::storage::WorkStore;

pub(crate) fn execute(store: &WorkStore) -> Result<CommandOutput> {
    Ok(CommandOutput::WorkList(store.list()?))
}
