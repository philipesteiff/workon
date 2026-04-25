use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::WorkSummary;

pub(crate) mod paths;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryAttachment {
    pub name_with_owner: String,
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
