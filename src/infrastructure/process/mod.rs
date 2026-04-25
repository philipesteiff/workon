use std::process::Command as ProcessCommand;

use crate::shared::error::{Result, WorkonError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoCommand {
    program: String,
    args: Vec<String>,
}

impl RepoCommand {
    pub(crate) fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub(crate) fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[cfg(test)]
    pub(crate) fn args_slice(&self) -> &[String] {
        &self.args
    }
}

pub(crate) trait ProcessRunner {
    fn run_checked(&self, command: &RepoCommand) -> Result<String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct StdProcessRunner;

impl ProcessRunner for StdProcessRunner {
    fn run_checked(&self, command: &RepoCommand) -> Result<String> {
        let mut process = ProcessCommand::new(&command.program);
        process.args(&command.args);
        let output = process.output()?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        Err(WorkonError::ProcessFailed {
            command: command.display(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}
