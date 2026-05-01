use std::path::{Path, PathBuf};

use crate::application::CommandOutput;
use crate::domain::{
    RepositoryCandidateInspection, RepositoryCandidateList, RepositoryCandidatePath,
    RepositoryCandidatePathList, RepositoryWorkspace, RepositoryWorkspaceList,
};
use crate::infrastructure::git_worktree::WorktreeInspector;
use crate::infrastructure::storage::{
    JsonRepositoryCandidateCache, RepoMetadataStore, RepoWorkspaceStore, WorkStore,
};
use crate::shared::error::{Result, WorkonError};

use super::paths::{
    normalize_existing_or_input_path, normalize_workspace_path, path_contains,
    repository_target_path,
};

pub(super) fn list(workspaces: &dyn RepoWorkspaceStore) -> Result<CommandOutput> {
    Ok(CommandOutput::RepositoryWorkspaces(
        RepositoryWorkspaceList {
            workspaces: workspaces.read()?,
        },
    ))
}

pub(super) fn add(workspaces: &dyn RepoWorkspaceStore, paths: &[PathBuf]) -> Result<CommandOutput> {
    if paths.is_empty() {
        return Err(WorkonError::MissingArgument {
            message: "repos workspace add requires at least one path".to_string(),
        });
    }

    let mut existing = workspaces.read()?;
    for path in paths {
        let workspace = normalize_workspace_path(path)?;
        if !existing.iter().any(|item| item.path == workspace.path) {
            existing.push(workspace);
        }
    }
    existing.sort_by(|left, right| left.path.cmp(&right.path));
    existing.dedup_by(|left, right| left.path == right.path);
    workspaces.write(&existing)?;
    list(workspaces)
}

pub(super) fn remove(
    store: &WorkStore,
    workspaces: &dyn RepoWorkspaceStore,
    metadata: &dyn RepoMetadataStore,
    path: &Path,
) -> Result<CommandOutput> {
    let path = normalize_existing_or_input_path(path)?;
    let mut existing = workspaces.read()?;
    ensure_unused_by_active_work(store, metadata, &path)?;
    existing.retain(|workspace| workspace.path != path);
    workspaces.write(&existing)?;
    list(workspaces)
}

pub(super) fn discover(
    store: &WorkStore,
    workspaces: &dyn RepoWorkspaceStore,
    inspector: &dyn WorktreeInspector,
    query: &str,
) -> Result<CommandOutput> {
    let work = store.open(query)?;
    let roots = workspaces
        .read()?
        .into_iter()
        .map(|workspace| workspace.path)
        .collect::<Vec<_>>();
    Ok(CommandOutput::RepositoryCandidates(
        RepositoryCandidateList {
            work: work.into(),
            candidates: inspector.discover(&roots)?,
        },
    ))
}

pub(super) fn candidate_paths(
    store: &WorkStore,
    workspaces: &dyn RepoWorkspaceStore,
    inspector: &dyn WorktreeInspector,
    cache: &JsonRepositoryCandidateCache,
    query: &str,
) -> Result<CommandOutput> {
    let work = store.open(query)?;
    let roots = workspaces
        .read()?
        .into_iter()
        .map(|workspace| workspace.path)
        .collect::<Vec<_>>();
    let paths = inspector
        .discover_candidate_paths(&roots)?
        .into_iter()
        .map(|path| {
            let cached = cache.valid_candidate(&path)?;
            Ok(RepositoryCandidatePath { path, cached })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CommandOutput::RepositoryCandidatePaths(
        RepositoryCandidatePathList {
            work: work.into(),
            paths,
        },
    ))
}

pub(super) fn inspect_candidate(
    inspector: &dyn WorktreeInspector,
    cache: &JsonRepositoryCandidateCache,
    path: &Path,
    refresh: bool,
) -> Result<CommandOutput> {
    if !refresh {
        if let Some(candidate) = cache.valid_candidate(path)? {
            return Ok(CommandOutput::RepositoryCandidateInspection(
                RepositoryCandidateInspection {
                    path: path.to_path_buf(),
                    candidate: Some(candidate),
                    cached: true,
                },
            ));
        }
    }

    let candidate = inspector.inspect_candidate(path)?;
    match &candidate {
        Some(candidate) => cache.store(candidate)?,
        None => cache.remove(path)?,
    }
    Ok(CommandOutput::RepositoryCandidateInspection(
        RepositoryCandidateInspection {
            path: path.to_path_buf(),
            candidate,
            cached: false,
        },
    ))
}

pub(super) fn select(
    workspaces: &dyn RepoWorkspaceStore,
    selected: Option<&Path>,
) -> Result<RepositoryWorkspace> {
    let workspaces = workspaces.read()?;
    if let Some(selected) = selected {
        let selected = normalize_existing_or_input_path(selected)?;
        return workspaces
            .into_iter()
            .find(|workspace| workspace.path == selected)
            .ok_or_else(|| WorkonError::RepositoryContext {
                message: format!(
                    "repository workspace is not configured: {}",
                    selected.display()
                ),
            });
    }

    match workspaces.as_slice() {
        [workspace] => Ok(workspace.clone()),
        [] => Err(WorkonError::RepositoryContext {
            message:
                "no repository workspace configured. Add one with: wo repos workspace add <path>..."
                    .to_string(),
        }),
        _ => Err(WorkonError::RepositoryContext {
            message:
                "multiple repository workspaces configured. Choose one with --workspace <path>"
                    .to_string(),
        }),
    }
}

fn ensure_unused_by_active_work(
    store: &WorkStore,
    metadata: &dyn RepoMetadataStore,
    workspace_path: &Path,
) -> Result<()> {
    let workspace_path = normalize_existing_or_input_path(workspace_path)?;
    for work in store.list()?.works {
        for repository in metadata.read(&work.path)? {
            let Some(target_path) = repository_target_path(&work.path, &repository)? else {
                continue;
            };
            if path_contains(&workspace_path, &target_path) {
                return Err(WorkonError::RepositoryContext {
                    message: format!(
                        "cannot remove repo workspace {}; active Work {} uses {} at {}",
                        workspace_path.display(),
                        work.slug,
                        repository.name_with_owner,
                        target_path.display()
                    ),
                });
            }
        }
    }
    Ok(())
}
