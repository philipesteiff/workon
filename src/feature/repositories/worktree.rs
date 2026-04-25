use std::path::Path;

use crate::error::{Result, WorkonError};

use super::process::{ProcessRunner, RepoCommand};

pub(super) trait WorktreeManager {
    fn switch(
        &self,
        cache_path: &Path,
        worktree_path: &Path,
        branch: &str,
        default_branch: &str,
    ) -> Result<()>;
    fn remove(&self, cache_path: &Path, branch: &str, force: bool) -> Result<()>;
}

pub(super) struct Worktrunk<'a> {
    runner: &'a dyn ProcessRunner,
}

impl<'a> Worktrunk<'a> {
    pub(super) fn new(runner: &'a dyn ProcessRunner) -> Self {
        Self { runner }
    }
}

impl WorktreeManager for Worktrunk<'_> {
    fn switch(
        &self,
        cache_path: &Path,
        worktree_path: &Path,
        branch: &str,
        default_branch: &str,
    ) -> Result<()> {
        let create = RepoCommand::new("wt")
            .args([
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
            ])
            .env(
                "WORKTRUNK_WORKTREE_PATH",
                worktree_path.display().to_string(),
            );

        match self.runner.run_checked(&create) {
            Ok(_) => Ok(()),
            Err(WorkonError::ProcessFailed { stderr, .. })
                if stderr.contains("already exists") || stderr.contains("cannot create branch") =>
            {
                let existing = RepoCommand::new("wt")
                    .args([
                        "-C",
                        &cache_path.display().to_string(),
                        "switch",
                        branch,
                        "--format",
                        "json",
                        "--no-cd",
                        "--no-hooks",
                    ])
                    .env(
                        "WORKTRUNK_WORKTREE_PATH",
                        worktree_path.display().to_string(),
                    );
                self.runner.run_checked(&existing)?;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn remove(&self, cache_path: &Path, branch: &str, force: bool) -> Result<()> {
        let mut args = vec![
            "-C".to_string(),
            cache_path.display().to_string(),
            "remove".to_string(),
            "--no-delete-branch".to_string(),
            "--foreground".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        if force {
            args.push("--force".to_string());
        }
        args.push(branch.to_string());
        self.runner
            .run_checked(&RepoCommand::new("wt").args(args))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::Path;

    use crate::error::Result;

    use super::{WorktreeManager, Worktrunk};
    use crate::feature::repositories::process::{ProcessRunner, RepoCommand};

    #[test]
    fn switch_uses_worktrunk_path_env_and_create_branch_args() {
        let runner = RecordingRunner::default();
        let worktrunk = Worktrunk::new(&runner);

        worktrunk
            .switch(
                Path::new("/cache/openai/workon.git"),
                Path::new("/work/repos/openai__workon"),
                "workon/billing",
                "main",
            )
            .expect("switch should run");

        let commands = runner.commands.borrow();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].program(), "wt");
        assert_eq!(commands[0].args_slice()[0], "-C");
        assert!(commands[0].args_slice().contains(&"--create".to_string()));
        assert!(commands[0]
            .args_slice()
            .contains(&"workon/billing".to_string()));
        assert_eq!(
            commands[0].env_slice(),
            &[(
                "WORKTRUNK_WORKTREE_PATH".to_string(),
                "/work/repos/openai__workon".to_string()
            )]
        );
    }

    #[test]
    fn remove_keeps_branch_and_can_force() {
        let runner = RecordingRunner::default();
        let worktrunk = Worktrunk::new(&runner);

        worktrunk
            .remove(
                Path::new("/cache/openai/workon.git"),
                "workon/billing",
                true,
            )
            .expect("remove should run");

        let commands = runner.commands.borrow();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].program(), "wt");
        assert!(commands[0]
            .args_slice()
            .contains(&"--no-delete-branch".to_string()));
        assert!(commands[0].args_slice().contains(&"--force".to_string()));
        assert_eq!(
            commands[0].args_slice().last(),
            Some(&"workon/billing".to_string())
        );
    }

    #[derive(Default)]
    struct RecordingRunner {
        commands: RefCell<Vec<RepoCommand>>,
    }

    impl ProcessRunner for RecordingRunner {
        fn run_checked(&self, command: &RepoCommand) -> Result<String> {
            self.commands.borrow_mut().push(command.clone());
            Ok("{}".to_string())
        }
    }
}
