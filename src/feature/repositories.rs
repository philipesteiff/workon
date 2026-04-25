use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use serde::Deserialize;

use crate::agent_files::write_agent_files_with_repos;
use crate::app::CommandOutput;
use crate::domain::{
    AttachedRepository, AvailableRepository, RepositoryCatalog, RepositoryContextChange,
    WorkRepositoryList, WorkSummary,
};
use crate::error::{Result, WorkonError};
use crate::intents::IntentCatalog;
use crate::storage::WorkStore;

const REPOSITORY_METADATA_FILE: &str = "workon.repos.json";
const REPOSITORY_CACHE_ROOT: &str = "repo-cache";
const GITHUB_HOST: &str = "github.com";

pub(crate) fn list_available() -> Result<CommandOutput> {
    let output = run_checked(
        "gh",
        &[
            "repo",
            "list",
            "--no-archived",
            "--limit",
            "100",
            "--json",
            "nameWithOwner,defaultBranchRef,url,sshUrl",
        ],
        &[],
    )?;
    let mut repositories = parse_gh_repositories(&output)?;
    repositories.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
    Ok(CommandOutput::RepositoryCatalog(RepositoryCatalog {
        repositories,
    }))
}

pub(crate) fn list_attached(store: &WorkStore, query: &str) -> Result<CommandOutput> {
    let work = store.open(query)?;
    let repositories = read_attached_repositories(&work.path)?;
    Ok(CommandOutput::WorkRepositories(WorkRepositoryList {
        work: work.into(),
        repositories,
    }))
}

pub(crate) fn add(
    store: &WorkStore,
    intents: &IntentCatalog,
    query: &str,
    repositories: &[String],
) -> Result<CommandOutput> {
    let work = store.open(query)?;
    let requested = normalize_requested_repositories(repositories)?;
    let mut attached = read_attached_repositories(&work.path)?;
    let mut changed = Vec::new();

    for name_with_owner in requested {
        if let Some(existing) = attached
            .iter()
            .find(|repo| repo.name_with_owner == name_with_owner)
            .cloned()
        {
            changed.push(existing);
            continue;
        }

        let available = fetch_repository(&name_with_owner)?;
        let cache_path = ensure_bare_cache(store.root(), &available)?;
        let branch = work_branch(&work);
        let path = worktree_path(&work, &available.name_with_owner)?;
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| WorkonError::RepositoryContext {
                    message: format!("repository path has no parent: {}", path.display()),
                })?,
        )?;

        switch_worktree(&cache_path, &path, &branch, &available.default_branch)?;

        let repository = AttachedRepository {
            name_with_owner: available.name_with_owner,
            branch,
            path,
            default_branch: available.default_branch,
            url: available.url,
        };
        attached.push(repository.clone());
        changed.push(repository);
    }

    attached.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
    write_attached_repositories(&work.path, &attached)?;
    rewrite_agent_files(intents, &work.clone().into(), &attached)?;

    Ok(CommandOutput::WorkRepositoriesAdded(
        RepositoryContextChange {
            work: work.into(),
            repositories: changed,
        },
    ))
}

pub(crate) fn remove(
    store: &WorkStore,
    intents: &IntentCatalog,
    query: &str,
    repositories: &[String],
) -> Result<CommandOutput> {
    let work = store.open(query)?;
    let requested = normalize_requested_repositories(repositories)?;
    let attached = read_attached_repositories(&work.path)?;
    let mut removed = Vec::new();

    for name_with_owner in &requested {
        let Some(repository) = attached
            .iter()
            .find(|repo| &repo.name_with_owner == name_with_owner)
            .cloned()
        else {
            continue;
        };
        let cache_path = repository_cache_path(store.root(), &repository.name_with_owner)?;
        remove_worktree(&cache_path, &repository.branch)?;
        removed.push(repository);
    }

    let remaining = attached
        .into_iter()
        .filter(|repo| {
            !removed
                .iter()
                .any(|removed| removed.name_with_owner == repo.name_with_owner)
        })
        .collect::<Vec<_>>();

    write_attached_repositories(&work.path, &remaining)?;
    rewrite_agent_files(intents, &work.clone().into(), &remaining)?;

    Ok(CommandOutput::WorkRepositoriesRemoved(
        RepositoryContextChange {
            work: work.into(),
            repositories: removed,
        },
    ))
}

fn fetch_repository(name_with_owner: &str) -> Result<AvailableRepository> {
    let output = run_checked(
        "gh",
        &[
            "repo",
            "view",
            name_with_owner,
            "--json",
            "nameWithOwner,defaultBranchRef,url,sshUrl",
        ],
        &[],
    )?;
    parse_gh_repository(&output)
}

fn ensure_bare_cache(root: &Path, repository: &AvailableRepository) -> Result<PathBuf> {
    let cache_path = repository_cache_path(root, &repository.name_with_owner)?;
    if cache_path.exists() {
        run_checked(
            "git",
            &["-C", &cache_path.display().to_string(), "fetch", "--prune"],
            &[],
        )?;
        return Ok(cache_path);
    }

    fs::create_dir_all(
        cache_path
            .parent()
            .ok_or_else(|| WorkonError::RepositoryContext {
                message: format!(
                    "repository cache path has no parent: {}",
                    cache_path.display()
                ),
            })?,
    )?;
    run_checked(
        "gh",
        &[
            "repo",
            "clone",
            &repository.name_with_owner,
            &cache_path.display().to_string(),
            "--",
            "--bare",
        ],
        &[],
    )?;
    Ok(cache_path)
}

fn switch_worktree(
    cache_path: &Path,
    worktree_path: &Path,
    branch: &str,
    default_branch: &str,
) -> Result<()> {
    let args = [
        "-C",
        &cache_path.display().to_string(),
        "switch",
        "--create",
        branch,
        "--base",
        default_branch,
        "--format",
        "json",
        "--no-cd",
        "--no-hooks",
    ];
    match run_checked(
        "wt",
        &args,
        &[(
            "WORKTRUNK_WORKTREE_PATH",
            worktree_path.display().to_string(),
        )],
    ) {
        Ok(_) => Ok(()),
        Err(WorkonError::ProcessFailed { stderr, .. })
            if stderr.contains("already exists") || stderr.contains("cannot create branch") =>
        {
            run_checked(
                "wt",
                &[
                    "-C",
                    &cache_path.display().to_string(),
                    "switch",
                    branch,
                    "--format",
                    "json",
                    "--no-cd",
                    "--no-hooks",
                ],
                &[(
                    "WORKTRUNK_WORKTREE_PATH",
                    worktree_path.display().to_string(),
                )],
            )?;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn remove_worktree(cache_path: &Path, branch: &str) -> Result<()> {
    run_checked(
        "wt",
        &[
            "-C",
            &cache_path.display().to_string(),
            "remove",
            "--no-delete-branch",
            "--foreground",
            "--format",
            "json",
            branch,
        ],
        &[],
    )?;
    Ok(())
}

fn read_attached_repositories(work_path: &Path) -> Result<Vec<AttachedRepository>> {
    let path = work_path.join(REPOSITORY_METADATA_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&path)?;
    let repositories =
        serde_json::from_str(&content).map_err(|error| WorkonError::RepositoryContext {
            message: format!("invalid repository metadata at {}: {error}", path.display()),
        })?;
    Ok(repositories)
}

fn write_attached_repositories(
    work_path: &Path,
    repositories: &[AttachedRepository],
) -> Result<()> {
    let content = serde_json::to_string_pretty(repositories).map_err(|error| {
        WorkonError::RepositoryContext {
            message: format!("could not write repository metadata: {error}"),
        }
    })?;
    fs::write(
        work_path.join(REPOSITORY_METADATA_FILE),
        format!("{content}\n"),
    )?;
    Ok(())
}

fn rewrite_agent_files(
    intents: &IntentCatalog,
    work: &WorkSummary,
    repositories: &[AttachedRepository],
) -> Result<()> {
    let Some(intent) = intents.find(&work.intent_id) else {
        return Err(WorkonError::UnknownIntent {
            intent_id: work.intent_id.clone(),
            available: intents.available_ids(),
        });
    };
    write_agent_files_with_repos(&work.path, &work.goal, &intent, repositories)
}

fn normalize_requested_repositories(repositories: &[String]) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for repository in repositories {
        let repository = repository.trim();
        if repository.is_empty() || !repository.contains('/') {
            return Err(WorkonError::RepositoryContext {
                message: format!("expected GitHub repository as owner/name, got `{repository}`"),
            });
        }
        if seen.insert(repository.to_string()) {
            normalized.push(repository.to_string());
        }
    }
    Ok(normalized)
}

fn repository_cache_path(root: &Path, name_with_owner: &str) -> Result<PathBuf> {
    let (owner, repo) = split_repository_name(name_with_owner)?;
    Ok(root
        .join(".workon")
        .join(REPOSITORY_CACHE_ROOT)
        .join(GITHUB_HOST)
        .join(owner)
        .join(format!("{repo}.git")))
}

fn worktree_path(work: &crate::domain::OpenedWork, name_with_owner: &str) -> Result<PathBuf> {
    let (owner, repo) = split_repository_name(name_with_owner)?;
    Ok(work.path.join("repos").join(format!("{owner}__{repo}")))
}

fn split_repository_name(name_with_owner: &str) -> Result<(&str, &str)> {
    let Some((owner, repo)) = name_with_owner.split_once('/') else {
        return Err(WorkonError::RepositoryContext {
            message: format!("expected GitHub repository as owner/name, got `{name_with_owner}`"),
        });
    };
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return Err(WorkonError::RepositoryContext {
            message: format!("expected GitHub repository as owner/name, got `{name_with_owner}`"),
        });
    }
    Ok((owner, repo))
}

fn work_branch(work: &crate::domain::OpenedWork) -> String {
    format!("workon/{}", work.slug)
}

fn run_checked(program: &str, args: &[&str], env: &[(&str, String)]) -> Result<String> {
    let mut command = ProcessCommand::new(program);
    command.args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command.output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    Err(WorkonError::ProcessFailed {
        command: display_command(program, args),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn display_command(program: &str, args: &[&str]) -> String {
    std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
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
