use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::{AvailableRepository, RepositoryAttachment};
use crate::infrastructure::process::{ProcessRunner, RepoCommand};
use crate::shared::error::{Result, WorkonError};

use super::linker::create_dir_symlink;
use super::{
    GitWorktree, RepositoryWorktreeCreateCommand, WorktreeInspector, WorktreeManager,
    WORKTREE_CREATE_COMMAND_ENV,
};

#[test]
fn switch_creates_branch_and_adds_worktree_with_raw_git() {
    let runner = RecordingRunner {
        current_branch_output: "workon/billing\n".to_string(),
        ..RecordingRunner::default()
    };
    let git = GitWorktree::new(&runner);
    let root = temp_root("git_worktree_switch");
    let worktree_path = root.path().join("repos/openai__workon");

    git.switch(
        &available_repository(),
        Path::new("/cache/openai/workon.git"),
        &worktree_path,
        "workon/billing",
    )
    .expect("switch should run");

    let commands = runner.commands.borrow();
    assert_eq!(commands.len(), 4);
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
    assert_eq!(commands[3].args_slice()[2], "branch");
    assert_eq!(commands[3].args_slice()[3], "--show-current");
}

#[test]
fn switch_reuses_existing_branch_without_creating_it() {
    let runner = RecordingRunner {
        branch_list_output: "workon/billing\n".to_string(),
        current_branch_output: "workon/billing\n".to_string(),
        ..RecordingRunner::default()
    };
    let git = GitWorktree::new(&runner);
    let root = temp_root("git_worktree_existing_branch");

    git.switch(
        &available_repository(),
        Path::new("/cache/openai/workon.git"),
        &root.path().join("repos/openai__workon"),
        "workon/billing",
    )
    .expect("switch should run");

    let commands = runner.commands.borrow();
    assert_eq!(commands.len(), 3);
    assert_args(&commands[0], branch_list_args());
    assert_eq!(commands[1].args_slice()[2], "worktree");
    assert_eq!(commands[2].args_slice()[2], "branch");
    assert_eq!(commands[2].args_slice()[3], "--show-current");
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
        &available_repository(),
        Path::new("/cache/openai/workon.git"),
        &worktree_path,
        "workon/billing",
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

#[test]
fn create_command_parses_and_expands_known_placeholders_without_reparsing_values() {
    let command = RepositoryWorktreeCreateCommand::parse(
        "worktrunk create --repo {repo} --path {target_path} --branch {branch}",
    )
    .expect("command template should parse");
    let root = temp_root("git_worktree_create_command_expands");
    let target_path = root.path().join("repo workspace/workon");

    let rendered = command
        .render(&available_repository(), &target_path, "workon/billing")
        .expect("command should render");

    assert_eq!(rendered.program(), "worktrunk");
    assert_eq!(
        rendered.args_slice(),
        expected_args(&[
            "create",
            "--repo",
            "openai/workon",
            "--path",
            target_path.to_str().expect("target path utf8"),
            "--branch",
            "workon/billing",
        ])
        .as_slice()
    );
}

#[test]
fn create_command_rejects_empty_templates() {
    let error =
        RepositoryWorktreeCreateCommand::parse("   ").expect_err("empty command should fail");

    assert_eq!(
        error.to_string(),
        format!("{WORKTREE_CREATE_COMMAND_ENV} cannot be empty")
    );
}

#[test]
fn create_command_rejects_unknown_placeholders() {
    let command = RepositoryWorktreeCreateCommand::parse("worktrunk create --repo {repository}")
        .expect("command template should parse");
    let error = command
        .render(
            &available_repository(),
            Path::new("/workspace/workon"),
            "workon/billing",
        )
        .expect_err("unknown placeholder should fail");

    assert_eq!(
        error.to_string(),
        "unknown placeholder `{repository}` in WORKON_REPOSITORY_WORKTREE_CREATE_COMMAND"
    );
}

#[test]
fn create_command_keeps_shell_syntax_as_arguments() {
    let command = RepositoryWorktreeCreateCommand::parse("worktrunk create '|' '>'")
        .expect("command template should parse");

    let rendered = command
        .render(
            &available_repository(),
            Path::new("/workspace/workon"),
            "workon/billing",
        )
        .expect("command should render");

    assert_eq!(
        rendered.args_slice(),
        expected_args(&["create", "|", ">"]).as_slice()
    );
}

#[test]
fn override_switch_runs_configured_create_command_instead_of_git_worktree_add() {
    let runner = RecordingRunner {
        current_branch_output: "workon/billing\n".to_string(),
        ..RecordingRunner::default()
    };
    let git = GitWorktree::with_create_command(
        &runner,
        RepositoryWorktreeCreateCommand::parse(
            "worktrunk create --repo {repo} --url {url} --ssh-url {ssh_url} --path {target_path} --branch {branch} --default {default_branch}",
        )
        .expect("command template should parse"),
    );
    let root = temp_root("git_worktree_override_switch");
    let worktree_path = root.path().join("repos/openai__workon");

    git.switch(
        &available_repository(),
        Path::new("/cache/openai/workon.git"),
        &worktree_path,
        "workon/billing",
    )
    .expect("switch should run");

    let commands = runner.commands.borrow();
    assert_eq!(commands.len(), 4);
    assert_args(&commands[0], branch_list_args());
    assert_eq!(
        commands[1].args_slice(),
        expected_args(&[
            "-C",
            "/cache/openai/workon.git",
            "branch",
            "workon/billing",
            "main",
        ])
        .as_slice()
    );
    assert_eq!(commands[2].program(), "worktrunk");
    assert_eq!(
        commands[2].args_slice(),
        expected_args(&[
            "create",
            "--repo",
            "openai/workon",
            "--url",
            "https://github.com/openai/workon",
            "--ssh-url",
            "git@github.com:openai/workon.git",
            "--path",
            worktree_path.to_str().expect("path utf8"),
            "--branch",
            "workon/billing",
            "--default",
            "main",
        ])
        .as_slice()
    );
    assert_eq!(commands[3].args_slice()[2], "branch");
    assert_eq!(commands[3].args_slice()[3], "--show-current");
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

fn available_repository() -> AvailableRepository {
    AvailableRepository {
        name_with_owner: "openai/workon".to_string(),
        default_branch: "main".to_string(),
        url: "https://github.com/openai/workon".to_string(),
        ssh_url: "git@github.com:openai/workon.git".to_string(),
    }
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
        if let Some(worktree_index) = args
            .windows(2)
            .position(|window| window == expected_args(&["worktree", "add"]).as_slice())
        {
            if let Some(path) = args.get(worktree_index + 2) {
                fs::create_dir_all(path).expect("recording runner should create worktree path");
            }
        }
        if command.program() == "worktrunk" {
            if let Some(path_index) = args.iter().position(|arg| arg == "--path") {
                if let Some(path) = args.get(path_index + 1) {
                    fs::create_dir_all(path).expect("recording runner should create override path");
                }
            }
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
