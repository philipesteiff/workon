use crate::domain::IntentCatalog;
use crate::domain::{AttachedRepository, WorkSummary};
use crate::infrastructure::agent_files::write_agent_files_with_repos;
use crate::shared::error::{Result, WorkonError};

pub(crate) trait RepoContextFileWriter {
    fn rewrite(&self, work: &WorkSummary, repositories: &[AttachedRepository]) -> Result<()>;
}

pub(crate) struct AgentRepoContextFileWriter<'a> {
    intents: &'a IntentCatalog,
}

impl<'a> AgentRepoContextFileWriter<'a> {
    pub(crate) fn new(intents: &'a IntentCatalog) -> Self {
        Self { intents }
    }
}

impl RepoContextFileWriter for AgentRepoContextFileWriter<'_> {
    fn rewrite(&self, work: &WorkSummary, repositories: &[AttachedRepository]) -> Result<()> {
        let Some(intent) = self.intents.find(&work.intent_id) else {
            return Err(WorkonError::UnknownIntent {
                intent_id: work.intent_id.clone(),
                available: self.intents.available_ids(),
            });
        };
        write_agent_files_with_repos(&work.path, &work.goal, &intent, repositories)
    }
}
