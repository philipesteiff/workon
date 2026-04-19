use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentProfile {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub skill_weights: Vec<String>,
    pub mcp_weights: Vec<String>,
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedWork {
    pub title: String,
    pub slug: String,
    pub goal: String,
    pub intent_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedWork {
    pub title: String,
    pub slug: String,
    pub goal: String,
    pub intent_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkSummary {
    pub title: String,
    pub slug: String,
    pub goal: String,
    pub intent_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkList {
    pub works: Vec<WorkSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextStatus {
    pub status: String,
    pub message: String,
}

impl From<WorkSummary> for OpenedWork {
    fn from(summary: WorkSummary) -> Self {
        Self {
            title: summary.title,
            slug: summary.slug,
            goal: summary.goal,
            intent_id: summary.intent_id,
            path: summary.path,
        }
    }
}

impl From<CreatedWork> for WorkSummary {
    fn from(work: CreatedWork) -> Self {
        Self {
            title: work.title,
            slug: work.slug,
            goal: work.goal,
            intent_id: work.intent_id,
            path: work.path,
        }
    }
}
