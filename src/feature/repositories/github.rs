use serde::Deserialize;
use std::path::Path;

use crate::domain::AvailableRepository;
use crate::error::{Result, WorkonError};

use super::process::{ProcessRunner, RepoCommand};

pub(super) trait GithubClient {
    fn list_repositories(&self) -> Result<Vec<AvailableRepository>>;
    fn fetch_repository(&self, name_with_owner: &str) -> Result<AvailableRepository>;
    fn clone_bare(&self, name_with_owner: &str, cache_path: &Path) -> Result<()>;
    fn fetch_cache(&self, cache_path: &Path) -> Result<()>;
}

pub(super) struct GhCli<'a> {
    runner: &'a dyn ProcessRunner,
}

impl<'a> GhCli<'a> {
    pub(super) fn new(runner: &'a dyn ProcessRunner) -> Self {
        Self { runner }
    }
}

impl GithubClient for GhCli<'_> {
    fn list_repositories(&self) -> Result<Vec<AvailableRepository>> {
        let output = self.runner.run_checked(&RepoCommand::new("gh").args([
            "repo",
            "list",
            "--no-archived",
            "--limit",
            "100",
            "--json",
            "nameWithOwner,defaultBranchRef,url,sshUrl",
        ]))?;
        let mut repositories = parse_gh_repositories(&output)?;
        repositories.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
        Ok(repositories)
    }

    fn fetch_repository(&self, name_with_owner: &str) -> Result<AvailableRepository> {
        let output = self.runner.run_checked(&RepoCommand::new("gh").args([
            "repo",
            "view",
            name_with_owner,
            "--json",
            "nameWithOwner,defaultBranchRef,url,sshUrl",
        ]))?;
        parse_gh_repository(&output)
    }

    fn clone_bare(&self, name_with_owner: &str, cache_path: &Path) -> Result<()> {
        self.runner.run_checked(&RepoCommand::new("gh").args([
            "repo",
            "clone",
            name_with_owner,
            &cache_path.display().to_string(),
            "--",
            "--bare",
        ]))?;
        Ok(())
    }

    fn fetch_cache(&self, cache_path: &Path) -> Result<()> {
        self.runner.run_checked(&RepoCommand::new("git").args([
            "-C",
            &cache_path.display().to_string(),
            "fetch",
            "--prune",
        ]))?;
        Ok(())
    }
}

fn parse_gh_repositories(output: &str) -> Result<Vec<AvailableRepository>> {
    let repositories: Vec<GhRepository> =
        serde_json::from_str(output).map_err(|error| WorkonError::RepositoryContext {
            message: format!("could not parse GitHub repository list: {error}"),
        })?;
    repositories
        .into_iter()
        .map(AvailableRepository::try_from)
        .collect()
}

fn parse_gh_repository(output: &str) -> Result<AvailableRepository> {
    let repository: GhRepository =
        serde_json::from_str(output).map_err(|error| WorkonError::RepositoryContext {
            message: format!("could not parse GitHub repository: {error}"),
        })?;
    AvailableRepository::try_from(repository)
}

#[derive(Debug, Deserialize)]
struct GhRepository {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
    #[serde(rename = "defaultBranchRef")]
    default_branch_ref: Option<GhBranch>,
    url: String,
    #[serde(rename = "sshUrl")]
    ssh_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhBranch {
    name: String,
}

impl TryFrom<GhRepository> for AvailableRepository {
    type Error = WorkonError;

    fn try_from(repository: GhRepository) -> Result<Self> {
        let default_branch = repository
            .default_branch_ref
            .map(|branch| branch.name)
            .ok_or_else(|| WorkonError::RepositoryContext {
                message: format!(
                    "GitHub repository `{}` has no default branch",
                    repository.name_with_owner
                ),
            })?;
        Ok(Self {
            name_with_owner: repository.name_with_owner,
            default_branch,
            url: repository.url,
            ssh_url: repository.ssh_url.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_gh_repositories, parse_gh_repository};

    #[test]
    fn parses_github_repository_catalog() {
        let repositories = parse_gh_repositories(
            r#"[
              {"nameWithOwner":"openai/workon","defaultBranchRef":{"name":"main"},"url":"https://github.com/openai/workon","sshUrl":"git@github.com:openai/workon.git"}
            ]"#,
        )
        .expect("catalog should parse");

        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].name_with_owner, "openai/workon");
        assert_eq!(repositories[0].default_branch, "main");
    }

    #[test]
    fn rejects_repositories_without_default_branch() {
        let error = parse_gh_repository(
            r#"{"nameWithOwner":"openai/empty","defaultBranchRef":null,"url":"https://github.com/openai/empty","sshUrl":null}"#,
        )
        .expect_err("default branch is required");

        assert!(error.to_string().contains("has no default branch"));
    }
}
