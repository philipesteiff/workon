use crate::application::CommandOutput;
use crate::infrastructure::storage::WorkStore;
use crate::shared::error::Result;

pub(crate) fn execute(store: &WorkStore) -> Result<CommandOutput> {
    Ok(CommandOutput::WorkList(store.list()?))
}
