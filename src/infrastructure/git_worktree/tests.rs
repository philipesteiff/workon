use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::RepositoryAttachment;
use crate::infrastructure::process::{ProcessRunner, RepoCommand};
use crate::shared::error::{Result, WorkonError};

use super::linker::create_dir_symlink;
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
fn scan_reconstructs_attached_repositories_from_repos_folder() {
    let runner = RecordingRunner {
        current_branch_output: "feature/manual-branch\n".to_string(),
        ..RecordingRunner::default()
    };
    let git = GitWorktree::new(&runner);
    let root = temp_root("git_worktree_scan");
    let repo_path = root.path().join("repos/workon");
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
        remote_url_output: "git@github.com:openai/workon.git\n".to_string(),
        ..RecordingRunner::default()
    };
    let git = GitWorktree::new(&runner);
    let root = temp_root("git_worktree_scan_detached");
    fs::create_dir_all(root.path().join("repos/workon")).expect("repo worktree path");

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
    fs::create_dir_all(root.path().join("repos/workon")).expect("repo worktree path");

    let repositories = git.scan(root.path(), &[]).expect("scan should succeed");

    assert_eq!(repositories.len(), 1);
    assert_eq!(repositories[0].url, "git@github.com:openai/workon.git");
    assert_eq!(repositories[0].default_branch, "");
}

#[test]
fn scan_skips_invalid_and_non_git_repo_folders() {
    let runner = RecordingRunner {
        branch_failure_contains: Some("notgit".to_string()),
        ..RecordingRunner::default()
    };
    let git = GitWorktree::new(&runner);
    let root = temp_root("git_worktree_scan_skips");
    fs::create_dir_all(root.path().join("repos/workon")).expect("repo worktree path");
    fs::create_dir_all(root.path().join("repos/notgit")).expect("non git path");
    fs::create_dir_all(root.path().join("repos/not-a-repo")).expect("invalid folder path");

    let repositories = git
        .scan(root.path(), &[repository_attachment()])
        .expect("scan should succeed");

    assert_eq!(repositories.len(), 1);
    assert_eq!(repositories[0].name_with_owner, "openai/workon");
}

#[test]
fn scan_reports_broken_repository_link_from_metadata() {
    let runner = RecordingRunner {
        branch_failure_contains: Some("workon".to_string()),
        ..RecordingRunner::default()
    };
    let git = GitWorktree::new(&runner);
    let root = temp_root("git_worktree_scan_broken_link");
    let repos_path = root.path().join("repos");
    let target_path = root.path().join("workspace/workon");
    let link_path = repos_path.join("workon");
    fs::create_dir_all(&repos_path).expect("repos path");
    create_dir_symlink(&target_path, &link_path).expect("broken repo link");

    let repositories = git
        .scan(root.path(), &[repository_attachment_at(&target_path)])
        .expect("scan should succeed");

    assert_eq!(repositories.len(), 1);
    assert_eq!(repositories[0].name_with_owner, "openai/workon");
    assert_eq!(repositories[0].branch, "missing");
    assert_eq!(repositories[0].path, link_path);
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
    repository_attachment_at(Path::new("/tmp/workon"))
}

fn repository_attachment_at(target_path: &Path) -> RepositoryAttachment {
    RepositoryAttachment {
        name_with_owner: "openai/workon".to_string(),
        default_branch: "main".to_string(),
        url: "https://github.com/openai/workon".to_string(),
        alias: "workon".to_string(),
        target_path: target_path.into(),
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
