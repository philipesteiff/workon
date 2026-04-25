use crate::application::work::create;
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
    match store.open(input) {
        Ok(work) => Ok(CommandOutput::WorkOpened(work)),
        Err(WorkonError::WorkNotFound { .. }) => {
            let Some(intent_id) = intent_id else {
                return Err(WorkonError::IntentRequired {
                    available: intents.available_ids(),
                });
            };
            create::execute(store, intents, input, intent_id)
        }
        Err(error) => Err(error),
    }
}
