use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::repository_context::paths::repository_owner_alias;
use crate::shared::error::{Result, WorkonError};

use super::RepositoryLinker;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SymlinkRepositoryLinker;

impl RepositoryLinker for SymlinkRepositoryLinker {
    fn link(
        &self,
        work_path: &Path,
        preferred_alias: &str,
        name_with_owner: &str,
        target_path: &Path,
    ) -> Result<PathBuf> {
        let repos_path = work_path.join("repos");
        fs::create_dir_all(&repos_path)?;
        let target = fs::canonicalize(target_path)?;
        let aliases = link_aliases(preferred_alias, name_with_owner)?;

        for alias in aliases {
            let link_path = repos_path.join(alias);
            if same_target(&link_path, &target) {
                return Ok(link_path);
            }
            if !link_path.exists() && fs::symlink_metadata(&link_path).is_err() {
                create_dir_symlink(&target, &link_path)?;
                return Ok(link_path);
            }
        }

        Err(WorkonError::RepositoryContext {
            message: format!(
                "could not choose repository link name for `{name_with_owner}` in {}",
                repos_path.display()
            ),
        })
    }

    fn remove(&self, link_path: &Path) -> Result<()> {
        let metadata = fs::symlink_metadata(link_path)?;
        if metadata.file_type().is_symlink() {
            fs::remove_file(link_path)?;
        } else if metadata.is_dir() {
            return Err(WorkonError::RepositoryContext {
                message: format!(
                    "refusing to remove real repository directory; expected Workon symlink: {}",
                    link_path.display()
                ),
            });
        } else {
            fs::remove_file(link_path)?;
        }
        Ok(())
    }
}

fn link_aliases(preferred_alias: &str, name_with_owner: &str) -> Result<Vec<String>> {
    let owner_alias = repository_owner_alias(name_with_owner)?;
    let preferred_alias = preferred_alias.trim();
    let preferred_alias = if preferred_alias.is_empty() {
        owner_alias.as_str()
    } else {
        preferred_alias
    };
    let mut aliases = vec![preferred_alias.to_string()];
    if preferred_alias != owner_alias {
        aliases.push(owner_alias.clone());
    }
    aliases.extend((2..10).map(|suffix| format!("{owner_alias}-{suffix}")));
    Ok(aliases)
}

fn same_target(link_path: &Path, target: &Path) -> bool {
    fs::canonicalize(link_path).is_ok_and(|existing| existing == target)
}

pub(super) fn symlink_target(link_path: &Path) -> std::io::Result<PathBuf> {
    let target = fs::read_link(link_path)?;
    if target.is_absolute() {
        Ok(target)
    } else {
        Ok(link_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target))
    }
}

#[cfg(unix)]
pub(super) fn create_dir_symlink(target: &Path, link_path: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link_path)?;
    Ok(())
}

#[cfg(windows)]
pub(super) fn create_dir_symlink(target: &Path, link_path: &Path) -> Result<()> {
    std::os::windows::fs::symlink_dir(target, link_path)?;
    Ok(())
}
