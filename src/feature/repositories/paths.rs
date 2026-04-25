use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::domain::OpenedWork;
use crate::error::{Result, WorkonError};

pub(super) const REPOSITORY_METADATA_FILE: &str = "workon.repos.json";

const REPOSITORY_CACHE_ROOT: &str = "repo-cache";
const GITHUB_HOST: &str = "github.com";

pub(super) fn normalize_requested_repositories(repositories: &[String]) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for repository in repositories {
        let repository = repository.trim();
        split_repository_name(repository)?;
        if seen.insert(repository.to_string()) {
            normalized.push(repository.to_string());
        }
    }
    Ok(normalized)
}

pub(super) fn repository_cache_path(root: &Path, name_with_owner: &str) -> Result<PathBuf> {
    let (owner, repo) = split_repository_name(name_with_owner)?;
    Ok(root
        .join(".workon")
        .join(REPOSITORY_CACHE_ROOT)
        .join(GITHUB_HOST)
        .join(owner)
        .join(format!("{repo}.git")))
}

pub(super) fn worktree_path(work: &OpenedWork, name_with_owner: &str) -> Result<PathBuf> {
    let (owner, repo) = split_repository_name(name_with_owner)?;
    Ok(work.path.join("repos").join(format!("{owner}__{repo}")))
}

pub(super) fn work_branch(work: &OpenedWork) -> String {
    format!("workon/{}", work.slug)
}

fn split_repository_name(name_with_owner: &str) -> Result<(&str, &str)> {
    let Some((owner, repo)) = name_with_owner.split_once('/') else {
        return Err(invalid_repository(name_with_owner));
    };
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return Err(invalid_repository(name_with_owner));
    }
    Ok((owner, repo))
}

fn invalid_repository(repository: &str) -> WorkonError {
    WorkonError::RepositoryContext {
        message: format!("expected GitHub repository as owner/name, got `{repository}`"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{normalize_requested_repositories, repository_cache_path};

    #[test]
    fn normalizes_requested_repositories_without_duplicates() {
        let repositories = normalize_requested_repositories(&[
            " openai/workon ".to_string(),
            "openai/api".to_string(),
            "openai/workon".to_string(),
        ])
        .expect("repositories should normalize");

        assert_eq!(repositories, ["openai/workon", "openai/api"]);
    }

    #[test]
    fn rejects_invalid_repository_names() {
        let error = normalize_requested_repositories(&["openai/api/extra".to_string()])
            .expect_err("nested repo names should fail");

        assert!(error.to_string().contains("expected GitHub repository"));
    }

    #[test]
    fn cache_path_is_keyed_by_host_owner_and_repo() {
        let path = repository_cache_path(Path::new("/tmp/workon"), "openai/workon")
            .expect("cache path should build");

        assert_eq!(
            path,
            Path::new("/tmp/workon/.workon/repo-cache/github.com/openai/workon.git")
        );
    }
}
