use std::path::{Path, PathBuf};

use crate::domain::repository_context::paths::repository_alias;
use crate::domain::{RepositoryAttachment, RepositoryWorkspace};
use crate::shared::error::{Result, WorkonError};

pub(super) fn normalize_workspace_path(path: &Path) -> Result<RepositoryWorkspace> {
    let path = expand_home_path(path)?;
    std::fs::create_dir_all(&path)?;
    Ok(RepositoryWorkspace {
        path: canonicalize(&path)?,
    })
}

pub(super) fn normalize_existing_or_input_path(path: &Path) -> Result<PathBuf> {
    let path = expand_home_path(path)?;
    if path.exists() {
        canonicalize(&path)
    } else {
        Ok(path)
    }
}

pub(super) fn canonicalize(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).map_err(Into::into)
}

pub(super) fn link_alias(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToString::to_string)
        .ok_or_else(|| WorkonError::RepositoryContext {
            message: format!("repository link has no valid alias: {}", path.display()),
        })
}

pub(super) fn repository_target_path(
    work_path: &Path,
    repository: &RepositoryAttachment,
) -> Result<Option<PathBuf>> {
    if !repository.target_path.as_os_str().is_empty() {
        return Ok(Some(normalize_existing_or_input_path(
            &repository.target_path,
        )?));
    }

    let alias = if repository.alias.is_empty() {
        repository_alias(&repository.name_with_owner)?
    } else {
        repository.alias.clone()
    };
    let link_path = work_path.join("repos").join(alias);
    if !link_path.exists() {
        return Ok(None);
    }
    let target_path = canonicalize(&link_path).or_else(|_| symlink_target(&link_path))?;
    Ok(Some(normalize_existing_or_input_path(&target_path)?))
}

pub(super) fn path_contains(parent: &Path, child: &Path) -> bool {
    child == parent || child.starts_with(parent)
}

fn expand_home_path(path: &Path) -> Result<PathBuf> {
    let Some(value) = path.to_str() else {
        return Ok(path.to_path_buf());
    };
    if value == "~" {
        return home_dir();
    }
    let Some(rest) = value.strip_prefix("~/") else {
        return Ok(path.to_path_buf());
    };
    Ok(home_dir()?.join(rest))
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| WorkonError::RepositoryContext {
            message: "HOME is required to expand repo workspace paths".to_string(),
        })
}

fn symlink_target(path: &Path) -> Result<PathBuf> {
    let target = std::fs::read_link(path)?;
    if target.is_absolute() {
        return Ok(target);
    }
    Ok(path
        .parent()
        .map(|parent| parent.join(&target))
        .unwrap_or(target))
}
