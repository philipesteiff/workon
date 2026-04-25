use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
pub struct ArchivedWork {
    pub title: String,
    pub slug: String,
    pub goal: String,
    pub intent_id: String,
    pub path: PathBuf,
    pub archive_path: PathBuf,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableRepository {
    pub name_with_owner: String,
    pub default_branch: String,
    pub url: String,
    pub ssh_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryCatalog {
    pub repositories: Vec<AvailableRepository>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachedRepository {
    pub name_with_owner: String,
    pub branch: String,
    pub path: PathBuf,
    pub default_branch: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkRepositoryList {
    pub work: WorkSummary,
    pub repositories: Vec<AttachedRepository>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryContextChange {
    pub work: WorkSummary,
    pub repositories: Vec<AttachedRepository>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbiguousWorkMatch {
    pub slug: String,
    pub title: String,
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

impl From<OpenedWork> for WorkSummary {
    fn from(work: OpenedWork) -> Self {
        Self {
            title: work.title,
            slug: work.slug,
            goal: work.goal,
            intent_id: work.intent_id,
            path: work.path,
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

impl From<WorkSummary> for AmbiguousWorkMatch {
    fn from(summary: WorkSummary) -> Self {
        Self {
            slug: summary.slug,
            title: summary.title,
        }
    }
}
