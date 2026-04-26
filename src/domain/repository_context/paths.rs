use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::domain::OpenedWork;
use crate::shared::error::{Result, WorkonError};

pub(crate) const REPOSITORY_METADATA_FILE: &str = "workon.repos.json";

const REPOSITORY_CACHE_ROOT: &str = "repo-cache";
const GITHUB_HOST: &str = "github.com";

pub(crate) fn normalize_requested_repositories(repositories: &[String]) -> Result<Vec<String>> {
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

pub(crate) fn repository_cache_path(root: &Path, name_with_owner: &str) -> Result<PathBuf> {
    let (owner, repo) = split_repository_name(name_with_owner)?;
    Ok(root
        .join(".workon")
        .join(REPOSITORY_CACHE_ROOT)
        .join(GITHUB_HOST)
        .join(owner)
        .join(format!("{repo}.git")))
}

pub(crate) fn workspace_worktree_path(
    workspace: &Path,
    work_slug: &str,
    name_with_owner: &str,
) -> Result<PathBuf> {
    Ok(workspace
        .join(work_slug)
        .join(repository_alias(name_with_owner)?))
}

pub(crate) fn repository_alias(name_with_owner: &str) -> Result<String> {
    let (_, repo) = split_repository_name(name_with_owner)?;
    Ok(repo.to_string())
}

pub(crate) fn repository_owner_alias(name_with_owner: &str) -> Result<String> {
    let (owner, repo) = split_repository_name(name_with_owner)?;
    Ok(format!("{owner}-{repo}"))
}

pub(crate) fn repository_name_from_remote_url(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches(".git");
    if let Some(path) = url.strip_prefix("git@github.com:") {
        return normalize_remote_path(path);
    }
    if let Some(path) = url.strip_prefix("https://github.com/") {
        return normalize_remote_path(path);
    }
    if let Some(path) = url.strip_prefix("ssh://git@github.com/") {
        return normalize_remote_path(path);
    }
    None
}

pub(crate) fn work_branch(work: &OpenedWork) -> String {
    format!("workon/{}", work.slug)
}

fn normalize_remote_path(path: &str) -> Option<String> {
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return None;
    }
    Some(format!("{}/{}", parts[0], parts[1]))
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

    use super::{
        normalize_requested_repositories, repository_alias, repository_cache_path,
        repository_name_from_remote_url, repository_owner_alias, workspace_worktree_path,
    };

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

    #[test]
    fn builds_repo_aliases_and_workspace_paths() {
        assert_eq!(
            repository_alias("openai/workon").expect("alias should build"),
            "workon"
        );
        assert_eq!(
            repository_owner_alias("openai/workon").expect("owner alias should build"),
            "openai-workon"
        );
        assert_eq!(
            workspace_worktree_path(Path::new("/repos"), "billing", "openai/workon")
                .expect("path should build"),
            Path::new("/repos/billing/workon")
        );
    }

    #[test]
    fn reconstructs_repository_name_from_remote_url() {
        assert_eq!(
            repository_name_from_remote_url("git@github.com:openai/workon.git"),
            Some("openai/workon".to_string())
        );
        assert_eq!(
            repository_name_from_remote_url("https://github.com/openai/workon"),
            Some("openai/workon".to_string())
        );
        assert_eq!(
            repository_name_from_remote_url("ssh://git@github.com/openai/workon.git"),
            Some("openai/workon".to_string())
        );
        assert_eq!(
            repository_name_from_remote_url("https://gitlab.com/openai/workon"),
            None
        );
    }
}
