use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::domain::repository_context::paths::repository_name_from_worktree_dir;
use crate::domain::{AttachedRepository, RepositoryAttachment};
use crate::infrastructure::process::{ProcessRunner, RepoCommand};
use crate::shared::error::{Result, WorkonError};

pub(crate) trait WorktreeManager {
    fn switch(
        &self,
        cache_path: &Path,
        worktree_path: &Path,
        branch: &str,
        default_branch: &str,
    ) -> Result<()>;
    fn remove(&self, cache_path: &Path, worktree_path: &Path, force: bool) -> Result<()>;
}

pub(crate) trait WorktreeInspector {
    fn scan(
        &self,
        work_path: &Path,
        metadata: &[RepositoryAttachment],
    ) -> Result<Vec<AttachedRepository>>;
}

pub(crate) struct GitWorktree<'a> {
    runner: &'a dyn ProcessRunner,
}

impl<'a> GitWorktree<'a> {
    pub(crate) fn new(runner: &'a dyn ProcessRunner) -> Self {
        Self { runner }
    }

    fn ensure_branch(&self, cache_path: &Path, branch: &str, default_branch: &str) -> Result<()> {
        let branches = self.runner.run_checked(&RepoCommand::new("git").args([
            "-C",
            &cache_path.display().to_string(),
            "branch",
            "--list",
            "--format=%(refname:short)",
            branch,
        ]))?;

        if branches.lines().any(|line| line.trim() == branch) {
            return Ok(());
        }

        self.runner.run_checked(&RepoCommand::new("git").args([
            "-C",
            &cache_path.display().to_string(),
            "branch",
            branch,
            default_branch,
        ]))?;
        Ok(())
    }

    fn current_branch(&self, worktree_path: &Path) -> Result<String> {
        self.runner
            .run_checked(&RepoCommand::new("git").args([
                "-C",
                &worktree_path.display().to_string(),
                "branch",
                "--show-current",
            ]))
            .map(|output| output.trim().to_string())
    }

    fn scan_branch(&self, worktree_path: &Path) -> Option<String> {
        let branch = self.current_branch(worktree_path).ok()?;
        if !branch.is_empty() {
            return Some(branch);
        }

        let revision = self
            .runner
            .run_checked(&RepoCommand::new("git").args([
                "-C",
                &worktree_path.display().to_string(),
                "rev-parse",
                "--short",
                "HEAD",
            ]))
            .ok()?;
        let revision = revision.trim();
        Some(if revision.is_empty() {
            "detached".to_string()
        } else {
            format!("detached {revision}")
        })
    }

    fn origin_url(&self, worktree_path: &Path) -> Option<String> {
        let url = self
            .runner
            .run_checked(&RepoCommand::new("git").args([
                "-C",
                &worktree_path.display().to_string(),
                "remote",
                "get-url",
                "origin",
            ]))
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

impl WorktreeInspector for GitWorktree<'_> {
    fn scan(
        &self,
        work_path: &Path,
        metadata: &[RepositoryAttachment],
    ) -> Result<Vec<AttachedRepository>> {
        let repos_path = work_path.join("repos");
        if !repos_path.exists() {
            return Ok(Vec::new());
        }

        let metadata_by_name = metadata
            .iter()
            .map(|repository| (repository.name_with_owner.as_str(), repository))
            .collect::<BTreeMap<_, _>>();
        let mut repositories = Vec::new();

        for entry in fs::read_dir(repos_path)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let folder_name = entry.file_name();
            let Some(name_with_owner) =
                repository_name_from_worktree_dir(&folder_name.to_string_lossy())
            else {
                continue;
            };
            let path = entry.path();
            let Some(branch) = self.scan_branch(&path) else {
                continue;
            };
            let metadata = metadata_by_name.get(name_with_owner.as_str()).copied();
            let url = metadata
                .map(|repository| repository.url.clone())
                .or_else(|| self.origin_url(&path))
                .unwrap_or_default();
            let default_branch = metadata
                .map(|repository| repository.default_branch.clone())
                .unwrap_or_default();

            repositories.push(AttachedRepository {
                name_with_owner,
                branch,
                path,
                default_branch,
                url,
            });
        }

        repositories.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
        Ok(repositories)
    }
}

impl WorktreeManager for GitWorktree<'_> {
    fn switch(
        &self,
        cache_path: &Path,
        worktree_path: &Path,
        branch: &str,
        default_branch: &str,
    ) -> Result<()> {
        fs::create_dir_all(worktree_path.parent().ok_or_else(|| {
            WorkonError::RepositoryContext {
                message: format!("repository path has no parent: {}", worktree_path.display()),
            }
        })?)?;

        if self.ensure_existing_worktree(worktree_path, branch)? {
            return Ok(());
        }

        self.ensure_branch(cache_path, branch, default_branch)?;
        self.runner.run_checked(&RepoCommand::new("git").args([
            "-C",
            &cache_path.display().to_string(),
            "worktree",
            "add",
            &worktree_path.display().to_string(),
            branch,
        ]))?;
        Ok(())
    }

    fn remove(&self, cache_path: &Path, worktree_path: &Path, force: bool) -> Result<()> {
        let mut args = vec![
            "-C".to_string(),
            cache_path.display().to_string(),
            "worktree".to_string(),
            "remove".to_string(),
        ];
        if force {
            args.push("--force".to_string());
        }
        args.push(worktree_path.display().to_string());

        self.runner
            .run_checked(&RepoCommand::new("git").args(args))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::domain::RepositoryAttachment;
    use crate::infrastructure::process::{ProcessRunner, RepoCommand};
    use crate::shared::error::{Result, WorkonError};

    use super::{GitWorktree, WorktreeInspector, WorktreeManager};

    #[test]
    fn switch_creates_branch_and_adds_worktree_with_raw_git() {
        let runner = RecordingRunner::default();
        let git = GitWorktree::new(&runner);
        let root = temp_root("git_worktree_switch");
        let worktree_path = root.path().join("repos/openai__workon");

        git.switch(
            Path::new("/cache/openai/workon.git"),
            &worktree_path,
            "workon/billing",
            "main",
        )
        .expect("switch should run");

        let commands = runner.commands.borrow();
        assert_eq!(commands.len(), 3);
        assert_args(&commands[0], branch_list_args());
        assert_eq!(
            commands[1].args_slice(),
            expected_args(&[
                "-C",
                "/cache/openai/workon.git",
                "branch",
                "workon/billing",
                "main"
            ])
            .as_slice()
        );
        assert_eq!(
            commands[2].args_slice(),
            expected_args(&[
                "-C",
                "/cache/openai/workon.git",
                "worktree",
                "add",
                worktree_path.to_str().expect("path utf8"),
                "workon/billing"
            ])
            .as_slice()
        );
    }

    #[test]
    fn switch_reuses_existing_branch_without_creating_it() {
        let runner = RecordingRunner {
            branch_list_output: "workon/billing\n".to_string(),
            ..RecordingRunner::default()
        };
        let git = GitWorktree::new(&runner);
        let root = temp_root("git_worktree_existing_branch");

        git.switch(
            Path::new("/cache/openai/workon.git"),
            &root.path().join("repos/openai__workon"),
            "workon/billing",
            "main",
        )
        .expect("switch should run");

        let commands = runner.commands.borrow();
        assert_eq!(commands.len(), 2);
        assert_args(&commands[0], branch_list_args());
        assert_eq!(commands[1].args_slice()[2], "worktree");
    }

    #[test]
    fn switch_accepts_existing_worktree_on_expected_branch() {
        let runner = RecordingRunner {
            current_branch_output: "workon/billing\n".to_string(),
            ..RecordingRunner::default()
        };
        let git = GitWorktree::new(&runner);
        let root = temp_root("git_worktree_existing_path");
        let worktree_path = root.path().join("repos/openai__workon");
        fs::create_dir_all(&worktree_path).expect("existing worktree path");

        git.switch(
            Path::new("/cache/openai/workon.git"),
            &worktree_path,
            "workon/billing",
            "main",
        )
        .expect("existing expected worktree should be accepted");

        let commands = runner.commands.borrow();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].args_slice()[2], "branch");
        assert_eq!(commands[0].args_slice()[3], "--show-current");
    }

    #[test]
    fn remove_uses_worktree_path_and_can_force() {
        let runner = RecordingRunner::default();
        let git = GitWorktree::new(&runner);

        git.remove(
            Path::new("/cache/openai/workon.git"),
            Path::new("/work/repos/openai__workon"),
            true,
        )
        .expect("remove should run");

        let commands = runner.commands.borrow();
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0].args_slice(),
            expected_args(&[
                "-C",
                "/cache/openai/workon.git",
                "worktree",
                "remove",
                "--force",
                "/work/repos/openai__workon"
            ])
            .as_slice()
        );
    }

    #[test]
    fn remove_propagates_dirty_worktree_failure() {
        let runner = RecordingRunner {
            remove_failure: true,
            ..RecordingRunner::default()
        };
        let git = GitWorktree::new(&runner);

        let error = git
            .remove(
                Path::new("/cache/openai/workon.git"),
                Path::new("/work/repos/openai__workon"),
                false,
            )
            .expect_err("dirty remove should fail");

        assert!(error.to_string().contains("contains modified files"));
    }

    #[test]
    fn scan_reconstructs_attached_repositories_from_repos_folder() {
        let runner = RecordingRunner {
            current_branch_output: "feature/manual-branch\n".to_string(),
            ..RecordingRunner::default()
        };
        let git = GitWorktree::new(&runner);
        let root = temp_root("git_worktree_scan");
        let repo_path = root.path().join("repos/openai__workon");
        fs::create_dir_all(&repo_path).expect("repo worktree path");
        fs::create_dir_all(root.path().join("repos/not-a-repo")).expect("invalid folder path");

        let repositories = git
            .scan(root.path(), &[repository_attachment()])
            .expect("scan should succeed");

        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].name_with_owner, "openai/workon");
        assert_eq!(repositories[0].branch, "feature/manual-branch");
        assert_eq!(repositories[0].path, repo_path);
        assert_eq!(repositories[0].default_branch, "main");
        assert_eq!(repositories[0].url, "https://github.com/openai/workon");
    }

    #[test]
    fn scan_uses_detached_head_revision_when_current_branch_is_empty() {
        let runner = RecordingRunner {
            rev_parse_output: "abc1234\n".to_string(),
            ..RecordingRunner::default()
        };
        let git = GitWorktree::new(&runner);
        let root = temp_root("git_worktree_scan_detached");
        fs::create_dir_all(root.path().join("repos/openai__workon")).expect("repo worktree path");

        let repositories = git.scan(root.path(), &[]).expect("scan should succeed");

        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].branch, "detached abc1234");
    }

    #[test]
    fn scan_uses_origin_url_when_metadata_is_missing() {
        let runner = RecordingRunner {
            current_branch_output: "main\n".to_string(),
            remote_url_output: "git@github.com:openai/workon.git\n".to_string(),
            ..RecordingRunner::default()
        };
        let git = GitWorktree::new(&runner);
        let root = temp_root("git_worktree_scan_origin_url");
        fs::create_dir_all(root.path().join("repos/openai__workon")).expect("repo worktree path");

        let repositories = git.scan(root.path(), &[]).expect("scan should succeed");

        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].url, "git@github.com:openai/workon.git");
        assert_eq!(repositories[0].default_branch, "");
    }

    #[test]
    fn scan_skips_invalid_and_non_git_repo_folders() {
        let runner = RecordingRunner {
            branch_failure_contains: Some("openai__notgit".to_string()),
            ..RecordingRunner::default()
        };
        let git = GitWorktree::new(&runner);
        let root = temp_root("git_worktree_scan_skips");
        fs::create_dir_all(root.path().join("repos/openai__workon")).expect("repo worktree path");
        fs::create_dir_all(root.path().join("repos/openai__notgit")).expect("non git path");
        fs::create_dir_all(root.path().join("repos/not-a-repo")).expect("invalid folder path");

        let repositories = git
            .scan(root.path(), &[repository_attachment()])
            .expect("scan should succeed");

        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].name_with_owner, "openai/workon");
    }

    fn branch_list_args() -> &'static [&'static str] {
        &[
            "-C",
            "/cache/openai/workon.git",
            "branch",
            "--list",
            "--format=%(refname:short)",
            "workon/billing",
        ]
    }

    fn assert_args(command: &RepoCommand, expected: &[&str]) {
        assert_eq!(command.args_slice(), expected_args(expected).as_slice());
    }

    fn expected_args(expected: &[&str]) -> Vec<String> {
        expected.iter().map(|value| value.to_string()).collect()
    }

    fn repository_attachment() -> RepositoryAttachment {
        RepositoryAttachment {
            name_with_owner: "openai/workon".to_string(),
            default_branch: "main".to_string(),
            url: "https://github.com/openai/workon".to_string(),
        }
    }

    #[derive(Default)]
    struct RecordingRunner {
        commands: RefCell<Vec<RepoCommand>>,
        branch_list_output: String,
        current_branch_output: String,
        rev_parse_output: String,
        remote_url_output: String,
        branch_failure_contains: Option<String>,
        remove_failure: bool,
    }

    impl ProcessRunner for RecordingRunner {
        fn run_checked(&self, command: &RepoCommand) -> Result<String> {
            self.commands.borrow_mut().push(command.clone());
            let args = command.args_slice();
            if args.ends_with(
                expected_args(&[
                    "branch",
                    "--list",
                    "--format=%(refname:short)",
                    "workon/billing",
                ])
                .as_slice(),
            ) {
                return Ok(self.branch_list_output.clone());
            }
            if args.ends_with(expected_args(&["branch", "--show-current"]).as_slice()) {
                if self
                    .branch_failure_contains
                    .as_ref()
                    .is_some_and(|value| args.iter().any(|arg| arg.contains(value)))
                {
                    return Err(WorkonError::ProcessFailed {
                        command: "git branch --show-current".to_string(),
                        stderr: "not a git repository".to_string(),
                    });
                }
                return Ok(self.current_branch_output.clone());
            }
            if args.ends_with(expected_args(&["rev-parse", "--short", "HEAD"]).as_slice()) {
                return Ok(self.rev_parse_output.clone());
            }
            if args.ends_with(expected_args(&["remote", "get-url", "origin"]).as_slice()) {
                return Ok(self.remote_url_output.clone());
            }
            if self.remove_failure && args.contains(&"remove".to_string()) {
                return Err(WorkonError::ProcessFailed {
                    command: "git worktree remove".to_string(),
                    stderr: "contains modified files".to_string(),
                });
            }
            Ok(String::new())
        }
    }

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn temp_root(name: &str) -> TempRoot {
        let mut path = std::env::temp_dir();
        path.push(format!("workon-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp root should be created");
        TempRoot { path }
    }
}
