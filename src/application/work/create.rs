use crate::application::CommandOutput;
use crate::domain::IntentCatalog;
use crate::infrastructure::agent_files::write_agent_files;
use crate::infrastructure::storage::WorkStore;
use crate::shared::error::{Result, WorkonError};

pub(crate) fn execute(
    store: &WorkStore,
    intents: &IntentCatalog,
    goal: &str,
    intent_id: &str,
) -> Result<CommandOutput> {
    let goal = goal.trim();
    if goal.is_empty() {
        return Err(WorkonError::EmptyGoal);
    }

    let Some(intent) = intents.find(intent_id) else {
        return Err(WorkonError::UnknownIntent {
            intent_id: intent_id.to_string(),
            available: intents.available_ids(),
        });
    };

    let work = store.create(goal, &intent.id)?;
    write_agent_files(&work.path, goal, &intent)?;

    Ok(CommandOutput::WorkCreated(work))
}
