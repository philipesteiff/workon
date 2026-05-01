use std::path::Path;

use crate::domain::AvailableRepository;
use crate::infrastructure::process::{ProcessRunner, RepoCommand};
use crate::shared::error::{Result, WorkonError};

pub(crate) const WORKTREE_CREATE_COMMAND_ENV: &str = "WORKON_REPOSITORY_WORKTREE_CREATE_COMMAND";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositoryWorktreeCreateCommand {
    tokens: Vec<String>,
}

impl RepositoryWorktreeCreateCommand {
    pub(crate) fn from_env() -> Result<Option<Self>> {
        let Some(value) = std::env::var_os(WORKTREE_CREATE_COMMAND_ENV) else {
            return Ok(None);
        };
        let value = value
            .into_string()
            .map_err(|_| WorkonError::RepositoryContext {
                message: format!("{WORKTREE_CREATE_COMMAND_ENV} must be valid UTF-8"),
            })?;
        Self::parse(&value).map(Some)
    }

    pub(crate) fn parse(template: &str) -> Result<Self> {
        let tokens = split_template(template)?;
        if tokens.is_empty() {
            return Err(WorkonError::RepositoryContext {
                message: format!("{WORKTREE_CREATE_COMMAND_ENV} cannot be empty"),
            });
        }
        Ok(Self { tokens })
    }

    pub(crate) fn run(
        &self,
        runner: &dyn ProcessRunner,
        repository: &AvailableRepository,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<()> {
        runner.run_checked(&self.render(repository, worktree_path, branch)?)?;
        Ok(())
    }

    pub(crate) fn render(
        &self,
        repository: &AvailableRepository,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<RepoCommand> {
        let rendered = self
            .tokens
            .iter()
            .map(|token| render_token(token, repository, worktree_path, branch))
            .collect::<Result<Vec<_>>>()?;
        let mut rendered = rendered.into_iter();
        let program = rendered
            .next()
            .ok_or_else(|| WorkonError::RepositoryContext {
                message: format!("{WORKTREE_CREATE_COMMAND_ENV} cannot be empty"),
            })?;
        Ok(RepoCommand::new(program).args(rendered))
    }
}

fn split_template(template: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = template.chars().peekable();

    while let Some(character) = chars.next() {
        match (quote, character) {
            (Some(active), value) if value == active => quote = None,
            (None, '\'' | '"') => quote = Some(character),
            (None, value) if value.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            (_, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push('\\');
                }
            }
            (_, value) => current.push(value),
        }
    }

    if let Some(active) = quote {
        return Err(WorkonError::RepositoryContext {
            message: format!("unterminated `{active}` quote in {WORKTREE_CREATE_COMMAND_ENV}"),
        });
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

fn render_token(
    token: &str,
    repository: &AvailableRepository,
    worktree_path: &Path,
    branch: &str,
) -> Result<String> {
    let mut rendered = String::new();
    let mut rest = token;

    while let Some(open) = rest.find('{') {
        rendered.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('}') else {
            return Err(WorkonError::RepositoryContext {
                message: format!("unterminated placeholder in {WORKTREE_CREATE_COMMAND_ENV}"),
            });
        };
        let placeholder = &after_open[..close];
        rendered.push_str(placeholder_value(
            placeholder,
            repository,
            worktree_path,
            branch,
        )?);
        rest = &after_open[close + 1..];
    }

    if rest.contains('}') {
        return Err(WorkonError::RepositoryContext {
            message: format!("unmatched `}}` in {WORKTREE_CREATE_COMMAND_ENV}"),
        });
    }
    rendered.push_str(rest);
    Ok(rendered)
}

fn placeholder_value<'a>(
    placeholder: &str,
    repository: &'a AvailableRepository,
    worktree_path: &'a Path,
    branch: &'a str,
) -> Result<&'a str> {
    match placeholder {
        "repo" => Ok(&repository.name_with_owner),
        "url" => Ok(&repository.url),
        "ssh_url" => Ok(&repository.ssh_url),
        "target_path" => worktree_path
            .to_str()
            .ok_or_else(|| WorkonError::RepositoryContext {
                message: "repository worktree path must be valid UTF-8 for command override"
                    .to_string(),
            }),
        "branch" => Ok(branch),
        "default_branch" => Ok(&repository.default_branch),
        _ => Err(WorkonError::RepositoryContext {
            message: format!(
                "unknown placeholder `{{{placeholder}}}` in {WORKTREE_CREATE_COMMAND_ENV}"
            ),
        }),
    }
}
