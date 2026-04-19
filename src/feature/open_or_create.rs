use crate::app::CommandOutput;
use crate::error::{Result, WorkonError};
use crate::feature::create_work;
use crate::intents::IntentCatalog;
use crate::storage::WorkStore;

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
            create_work::execute(store, intents, input, intent_id)
        }
        Err(error) => Err(error),
    }
}
