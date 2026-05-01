use std::fs;
use std::path::Path;

use crate::domain::AvailableRepository;
use crate::infrastructure::process::{ProcessRunner, RepoCommand};
use crate::shared::error::{Result, WorkonError};

use super::{RepositoryWorktreeCreateCommand, WorktreeManager};

pub(crate) struct GitWorktree<'a> {
    runner: &'a dyn ProcessRunner,
    create_command: Option<RepositoryWorktreeCreateCommand>,
}

impl<'a> GitWorktree<'a> {
    pub(crate) fn new(runner: &'a dyn ProcessRunner) -> Self {
        Self {
            runner,
            create_command: None,
        }
    }

    pub(crate) fn from_env(runner: &'a dyn ProcessRunner) -> Result<Self> {
        Ok(Self {
            runner,
            create_command: RepositoryWorktreeCreateCommand::from_env()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_create_command(
        runner: &'a dyn ProcessRunner,
        create_command: RepositoryWorktreeCreateCommand,
    ) -> Self {
        Self {
            runner,
            create_command: Some(create_command),
        }
    }

    fn run_git(&self, repository_path: &Path, args: &[&str]) -> Result<String> {
        self.runner
            .run_checked(&git_command(repository_path, args.iter().copied()))
    }

    fn ensure_branch(&self, cache_path: &Path, branch: &str, default_branch: &str) -> Result<()> {
        let branches = self.run_git(
            cache_path,
            &["branch", "--list", "--format=%(refname:short)", branch],
        )?;

        if branches.lines().any(|line| line.trim() == branch) {
            return Ok(());
        }

        self.run_git(cache_path, &["branch", branch, default_branch])?;
        Ok(())
    }

    fn add_worktree(
        &self,
        repository: &AvailableRepository,
        cache_path: &Path,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<()> {
        if let Some(command) = &self.create_command {
            return command.run(self.runner, repository, worktree_path, branch);
        }

        self.run_git(
            cache_path,
            &[
                "worktree",
                "add",
                &worktree_path.display().to_string(),
                branch,
            ],
        )?;
        Ok(())
    }

    fn verify_created_worktree(&self, worktree_path: &Path, branch: &str) -> Result<()> {
        if !worktree_path.exists() {
            return Err(WorkonError::RepositoryContext {
                message: format!(
                    "repository worktree create command did not create a git worktree at {}",
                    worktree_path.display()
                ),
            });
        }

        let current_branch =
            self.current_branch(worktree_path)
                .map_err(|error| WorkonError::RepositoryContext {
                    message: format!(
                        "repository worktree create command did not create a git worktree at {} ({error})",
                        worktree_path.display()
                    ),
                })?;

        if current_branch == branch {
            return Ok(());
        }

        Err(WorkonError::RepositoryContext {
            message: format!(
                "repository worktree create command created `{}` on branch `{current_branch}`, expected `{branch}`",
                worktree_path.display()
            ),
        })
    }

    pub(super) fn current_branch(&self, worktree_path: &Path) -> Result<String> {
        self.run_git(worktree_path, &["branch", "--show-current"])
            .map(|output| output.trim().to_string())
    }

    pub(super) fn scan_branch(&self, worktree_path: &Path) -> Option<String> {
        let branch = self.current_branch(worktree_path).ok()?;
        if !branch.is_empty() {
            return Some(branch);
        }

        let revision = self
            .run_git(worktree_path, &["rev-parse", "--short", "HEAD"])
            .ok()?;
        let revision = revision.trim();
        Some(if revision.is_empty() {
            "detached".to_string()
        } else {
            format!("detached {revision}")
        })
    }

    pub(super) fn origin_url(&self, worktree_path: &Path) -> Option<String> {
        let url = self
            .run_git(worktree_path, &["remote", "get-url", "origin"])
            .ok()?;
        let url = url.trim().to_string();
        (!url.is_empty()).then_some(url)
    }

    fn ensure_existing_worktree(&self, worktree_path: &Path, branch: &str) -> Result<bool> {
        if !worktree_path.exists() {
            return Ok(false);
        }

        let current_branch =
            self.current_branch(worktree_path)
                .map_err(|error| WorkonError::RepositoryContext {
                    message: format!(
                        "repository path already exists but is not a git worktree: {} ({error})",
                        worktree_path.display()
                    ),
                })?;

        if current_branch == branch {
            return Ok(true);
        }

        Err(WorkonError::RepositoryContext {
            message: format!(
                "repository path already exists on branch `{current_branch}`, expected `{branch}`: {}",
                worktree_path.display()
            ),
        })
    }
}

impl WorktreeManager for GitWorktree<'_> {
    fn switch(
        &self,
        repository: &AvailableRepository,
        cache_path: &Path,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<()> {
        fs::create_dir_all(worktree_path.parent().ok_or_else(|| {
            WorkonError::RepositoryContext {
                message: format!("repository path has no parent: {}", worktree_path.display()),
            }
        })?)?;

        if self.ensure_existing_worktree(worktree_path, branch)? {
            return Ok(());
        }

        self.ensure_branch(cache_path, branch, &repository.default_branch)?;
        self.add_worktree(repository, cache_path, worktree_path, branch)?;
        self.verify_created_worktree(worktree_path, branch)?;
        Ok(())
    }
}

fn git_command<'a>(repository_path: &Path, args: impl IntoIterator<Item = &'a str>) -> RepoCommand {
    let mut command_args = vec!["-C".to_string(), repository_path.display().to_string()];
    command_args.extend(args.into_iter().map(str::to_string));
    RepoCommand::new("git").args(command_args)
}
