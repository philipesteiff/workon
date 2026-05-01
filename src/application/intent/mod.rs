use crate::application::CommandOutput;
use crate::domain::{
    IntentCatalog, IntentList, IntentProfileChange, IntentSource, WorkIntentSwitch, WorkSummary,
};
use crate::infrastructure::agent_files::{
    validate_agent_files_context_blocks, write_agent_files_with_repos_replacing_intent,
};
use crate::infrastructure::git_worktree::{GitWorktree, WorktreeInspector};
use crate::infrastructure::process::StdProcessRunner;
use crate::infrastructure::storage::{JsonRepoMetadataStore, RepoMetadataStore, WorkStore};
use crate::shared::error::{Result, WorkonError};

pub(crate) fn list(intents: &IntentCatalog) -> Result<CommandOutput> {
    Ok(CommandOutput::IntentList(IntentList {
        intents: intents.summaries(),
    }))
}

pub(crate) fn show(intents: &IntentCatalog, intent_id: &str) -> Result<CommandOutput> {
    let Some(intent) = intents.find(intent_id) else {
        return Err(WorkonError::UnknownIntent {
            intent_id: intent_id.to_string(),
            available: intents.available_ids(),
        });
    };

    Ok(CommandOutput::IntentShown(IntentProfileChange {
        source: intents.source(intent_id).unwrap_or(IntentSource::Default),
        intent,
    }))
}

pub(crate) fn switch_work_intent(
    store: &WorkStore,
    intents: &IntentCatalog,
    query: &str,
    intent_id: &str,
) -> Result<CommandOutput> {
    let Some(intent) = intents.find(intent_id) else {
        return Err(WorkonError::UnknownIntent {
            intent_id: intent_id.to_string(),
            available: intents.available_ids(),
        });
    };

    let current_work: WorkSummary = store.open(query)?.into();
    validate_agent_files_context_blocks(&current_work.path)?;

    let (previous_intent_id, work) = store.update_intent(query, &intent.id)?;
    let previous_intent = intents
        .find(&previous_intent_id)
        .unwrap_or_else(|| intent.clone());
    let metadata = JsonRepoMetadataStore;
    let runner = StdProcessRunner;
    let inspector = GitWorktree::new(&runner);
    let repository_metadata = metadata.read(&work.path)?;
    let repositories = inspector.scan(&work.path, &repository_metadata)?;
    write_agent_files_with_repos_replacing_intent(
        &work.path,
        &work.goal,
        &intent,
        &repositories,
        &previous_intent,
    )?;

    Ok(CommandOutput::WorkIntentSwitched(WorkIntentSwitch {
        work,
        previous_intent_id,
        intent,
    }))
}
