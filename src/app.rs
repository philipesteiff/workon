use std::path::PathBuf;

use crate::agent_files::write_agent_files;
use crate::domain::{ContextStatus, CreatedWork, OpenedWork, WorkList};
use crate::error::{Result, WorkonError};
use crate::intents::IntentCatalog;
use crate::storage::WorkStore;

#[derive(Debug, Clone)]
pub struct App {
    store: WorkStore,
    intents: IntentCatalog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Context,
    CreateWork {
        goal: String,
        intent_id: String,
    },
    ListWorks,
    OpenOrCreate {
        input: String,
        intent_id: Option<String>,
    },
    OpenWork {
        query: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutput {
    Context(ContextStatus),
    WorkCreated(CreatedWork),
    WorkList(WorkList),
    WorkOpened(OpenedWork),
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        Self {
            store: WorkStore::new(root),
            intents: IntentCatalog::default_catalog(),
        }
    }

    pub fn execute(&self, command: Command) -> Result<CommandOutput> {
        match command {
            Command::Context => Ok(CommandOutput::Context(context_status())),
            Command::CreateWork { goal, intent_id } => self.create_work(&goal, &intent_id),
            Command::ListWorks => Ok(CommandOutput::WorkList(self.store.list()?)),
            Command::OpenOrCreate { input, intent_id } => {
                self.open_or_create(&input, intent_id.as_deref())
            }
            Command::OpenWork { query } => Ok(CommandOutput::WorkOpened(self.store.open(&query)?)),
        }
    }

    pub fn available_intents(&self) -> Vec<(String, String)> {
        self.intents
            .profiles()
            .iter()
            .map(|intent| (intent.id.clone(), intent.summary.clone()))
            .collect()
    }

    fn open_or_create(&self, input: &str, intent_id: Option<&str>) -> Result<CommandOutput> {
        match self.store.open(input) {
            Ok(work) => Ok(CommandOutput::WorkOpened(work)),
            Err(WorkonError::WorkNotFound { .. }) => {
                let Some(intent_id) = intent_id else {
                    return Err(WorkonError::IntentRequired {
                        available: self.intents.available_ids(),
                    });
                };
                self.create_work(input, intent_id)
            }
            Err(error) => Err(error),
        }
    }

    fn create_work(&self, goal: &str, intent_id: &str) -> Result<CommandOutput> {
        let goal = goal.trim();
        if goal.is_empty() {
            return Err(WorkonError::EmptyGoal);
        }

        let Some(intent) = self.intents.find(intent_id) else {
            return Err(WorkonError::UnknownIntent {
                intent_id: intent_id.to_string(),
                available: self.intents.available_ids(),
            });
        };

        let work = self.store.create(goal, &intent.id)?;
        write_agent_files(&work.path, goal, &intent)?;

        Ok(CommandOutput::WorkCreated(work))
    }
}

fn context_status() -> ContextStatus {
    ContextStatus {
        status: "TBD".to_string(),
        message:
            "Context editing will change intent, skills, MCPs, and repos for the current work."
                .to_string(),
    }
}
