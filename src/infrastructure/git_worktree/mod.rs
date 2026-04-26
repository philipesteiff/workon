use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::repository_context::paths::{
    repository_name_from_remote_url, repository_owner_alias,
};
use crate::domain::{AttachedRepository, RepositoryAttachment, RepositoryCandidate};
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
}

pub(crate) trait WorktreeInspector {
    fn scan(
        &self,
        work_path: &Path,
        metadata: &[RepositoryAttachment],
    ) -> Result<Vec<AttachedRepository>>;
    fn inspect_candidate(&self, path: &Path) -> Result<Option<RepositoryCandidate>>;
    fn discover(&self, roots: &[PathBuf]) -> Result<Vec<RepositoryCandidate>>;
}

pub(crate) trait RepositoryLinker {
    fn link(
        &self,
        work_path: &Path,
        preferred_alias: &str,
        name_with_owner: &str,
        target_path: &Path,
    ) -> Result<PathBuf>;
    fn remove(&self, link_path: &Path) -> Result<()>;
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

    fn inspected_repository(
        &self,
        path: &Path,
        metadata: Option<&RepositoryAttachment>,
    ) -> Result<Option<AttachedRepository>> {
        let Some(branch) = self.scan_branch(path) else {
            return Ok(None);
        };
        let url = self
            .origin_url(path)
            .or_else(|| metadata.map(|repository| repository.url.clone()))
            .unwrap_or_default();
        let name_with_owner = repository_name_from_remote_url(&url)
            .or_else(|| metadata.map(|repository| repository.name_with_owner.clone()));
        let Some(name_with_owner) = name_with_owner else {
            return Ok(None);
        };
        let default_branch = metadata
            .map(|repository| repository.default_branch.clone())
            .unwrap_or_default();

        Ok(Some(AttachedRepository {
            name_with_owner,
            branch,
            path: path.to_path_buf(),
            default_branch,
            url,
        }))
    }

    fn candidate_at(&self, path: &Path) -> Result<Option<RepositoryCandidate>> {
        let Some(repository) = self.inspected_repository(path, None)? else {
            return Ok(None);
        };
        Ok(Some(RepositoryCandidate {
            name_with_owner: repository.name_with_owner,
            branch: repository.branch,
            path: path.to_path_buf(),
            url: repository.url,
        }))
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

        let metadata_by_alias = metadata
            .iter()
            .filter(|repository| !repository.alias.is_empty())
            .map(|repository| (repository.alias.as_str(), repository))
            .collect::<BTreeMap<_, _>>();
        let metadata_by_target = metadata
            .iter()
            .filter(|repository| !repository.target_path.as_os_str().is_empty())
            .map(|repository| (repository.target_path.as_path(), repository))
            .collect::<BTreeMap<_, _>>();
        let mut repositories = Vec::new();

        for entry in fs::read_dir(repos_path)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = fs::symlink_metadata(&path)?.file_type();
            let is_symlink = file_type.is_symlink();
            if !path.is_dir() && !is_symlink {
                continue;
            }

            let folder_name = entry.file_name();
            let alias = folder_name.to_string_lossy();
            let target = fs::canonicalize(&path)
                .or_else(|_| symlink_target(&path))
                .unwrap_or_else(|_| path.clone());
            let metadata = metadata_by_alias
                .get(alias.as_ref())
                .copied()
                .or_else(|| metadata_by_target.get(target.as_path()).copied());
            if let Some(repository) = self.inspected_repository(&path, metadata)? {
                repositories.push(repository);
            } else if is_symlink && !path.exists() {
                if let Some(metadata) = metadata {
                    repositories.push(missing_repository_link(&path, metadata));
                }
            }
        }

        repositories.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
        Ok(repositories)
    }

    fn inspect_candidate(&self, path: &Path) -> Result<Option<RepositoryCandidate>> {
        self.candidate_at(path)
    }

    fn discover(&self, roots: &[PathBuf]) -> Result<Vec<RepositoryCandidate>> {
        let mut candidates = Vec::new();
        for root in roots {
            discover_root(self, root, 0, &mut candidates)?;
        }
        candidates.sort_by(|left, right| {
            left.name_with_owner
                .cmp(&right.name_with_owner)
                .then(left.path.cmp(&right.path))
        });
        candidates.dedup_by(|left, right| left.path == right.path);
        Ok(candidates)
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
}

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

fn discover_root(
    git: &GitWorktree<'_>,
    path: &Path,
    depth: usize,
    candidates: &mut Vec<RepositoryCandidate>,
) -> Result<()> {
    const MAX_DEPTH: usize = 4;
    if depth > MAX_DEPTH || !path.exists() || ignored_discovery_path(path) {
        return Ok(());
    }

    if let Some(candidate) = git.candidate_at(path)? {
        candidates.push(candidate);
        return Ok(());
    }

    if !path.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            discover_root(git, &child, depth + 1, candidates)?;
        }
    }
    Ok(())
}

fn ignored_discovery_path(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | "node_modules" | "target" | ".cache" | ".workon")
    )
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

fn symlink_target(link_path: &Path) -> std::io::Result<PathBuf> {
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

fn missing_repository_link(path: &Path, metadata: &RepositoryAttachment) -> AttachedRepository {
    AttachedRepository {
        name_with_owner: metadata.name_with_owner.clone(),
        branch: "missing".to_string(),
        path: path.to_path_buf(),
        default_branch: metadata.default_branch.clone(),
        url: metadata.url.clone(),
    }
}

#[cfg(unix)]
fn create_dir_symlink(target: &Path, link_path: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link_path)?;
    Ok(())
}

#[cfg(windows)]
fn create_dir_symlink(target: &Path, link_path: &Path) -> Result<()> {
    std::os::windows::fs::symlink_dir(target, link_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::domain::RepositoryAttachment;
    use crate::infrastructure::process::{ProcessRunner, RepoCommand};
    use crate::shared::error::{Result, WorkonError};

    use super::{create_dir_symlink, GitWorktree, WorktreeInspector, WorktreeManager};

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
}
