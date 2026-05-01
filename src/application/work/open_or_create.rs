use crate::application::work::{create, open};
use crate::application::CommandOutput;
use crate::domain::IntentCatalog;
use crate::infrastructure::storage::WorkStore;
use crate::shared::error::{Result, WorkonError};

pub(crate) fn execute(
    store: &WorkStore,
    intents: &IntentCatalog,
    input: &str,
    intent_id: Option<&str>,
) -> Result<CommandOutput> {
    match open::execute(store, intents, input) {
        Ok(output) => Ok(output),
        Err(WorkonError::WorkNotFound { .. }) => {
            let intent_id = intent_id.unwrap_or(IntentCatalog::BLANK_INTENT_ID);
            create::execute(store, intents, input, intent_id)
        }
        Err(error) => Err(error),
    }
}
