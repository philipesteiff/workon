use crate::application::CommandOutput;
use crate::domain::IntentCatalog;
use crate::infrastructure::agent_files::write_agent_files_with_repos;
use crate::infrastructure::git_worktree::{GitWorktree, WorktreeInspector};
use crate::infrastructure::process::StdProcessRunner;
use crate::infrastructure::storage::{JsonRepoMetadataStore, RepoMetadataStore, WorkStore};
use crate::shared::error::{Result, WorkonError};

pub(crate) fn execute(
    store: &WorkStore,
    intents: &IntentCatalog,
    query: &str,
) -> Result<CommandOutput> {
    let work = store.open(query)?;
    let Some(intent) = intents.find(&work.intent_id) else {
        return Err(WorkonError::UnknownIntent {
            intent_id: work.intent_id.clone(),
            available: intents.available_ids(),
        });
    };

    let metadata = JsonRepoMetadataStore;
    let runner = StdProcessRunner;
    let inspector = GitWorktree::new(&runner);
    let repository_metadata = metadata.read(&work.path)?;
    let repositories = inspector.scan(&work.path, &repository_metadata)?;
    write_agent_files_with_repos(&work.path, &work.goal, &intent, &repositories)?;

    Ok(CommandOutput::WorkOpened(work))
}
