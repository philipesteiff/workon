use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use crate::error::{Result, WorkonError};

const WORKON_ACTIVE_ENV: &str = "WORKON_ACTIVE";
const WORKON_ROOT_ENV: &str = "WORKON_ROOT";
const WORKON_WORK_ENV: &str = "WORKON_WORK";
const WORKON_TITLE_ENV: &str = "WORKON_TITLE";
const WORKON_DEV_MANIFEST_ENV: &str = "WORKON_DEV_MANIFEST";
const WORKON_SHELL_COMMAND_ENV: &str = "WORKON_SHELL_COMMAND";
const WORKON_SKIP_USER_ZSHRC_ENV: &str = "WORKON_SKIP_USER_ZSHRC";

pub(crate) fn workon_root() -> std::io::Result<PathBuf> {
    match std::env::var(WORKON_ROOT_ENV) {
        Ok(root) => Ok(PathBuf::from(root)),
        Err(_) => std::env::current_dir(),
    }
}

pub(crate) fn is_work_shell_active() -> bool {
    std::env::var_os(WORKON_ACTIVE_ENV).is_some()
}

pub(crate) fn enter_work_shell(root: &Path, title: &str, work_path: &Path) -> Result<i32> {
    let rc_dir = TempDir::create("workon-zsh")?;
    let rc_path = rc_dir.path().join(".zshrc");
    fs::write(&rc_path, render_zshrc(root, title, work_path)?)?;

    let mut command = ProcessCommand::new(zsh_path());
    command
        .current_dir(work_path)
        .env("ZDOTDIR", rc_dir.path())
        .env(WORKON_ACTIVE_ENV, "1")
        .env(WORKON_ROOT_ENV, root)
        .env(WORKON_WORK_ENV, work_path)
        .env(WORKON_TITLE_ENV, title);

    if let Ok(shell_command) = std::env::var(WORKON_SHELL_COMMAND_ENV) {
        command.arg("-i").arg("-c").arg(shell_command);
    } else {
        command.arg("-i");
    }

    let status = command.status()?;
    Ok(status.code().unwrap_or(1))
}

fn render_zshrc(root: &Path, title: &str, work_path: &Path) -> Result<String> {
    let runner = shell_runner()?;

    Ok(format!(
        "\
if [ -z \"${{{skip_user_zshrc}:-}}\" ] && [ -n \"${{HOME:-}}\" ] && [ -f \"$HOME/.zshrc\" ]; then
  source \"$HOME/.zshrc\"
fi

export {active}=1
export {root_env}={root}
export {work_env}={work}
export {title_env}={title}

wo() {{
  local workon_output workon_status workon_switch_path workon_switch_title
  workon_output=\"$({active}=1 {root_env}={root} {runner} 2>&1)\"
  workon_status=$?
  printf '%s\\n' \"$workon_output\" | sed '/^__WORKON_SWITCH_/d'
  workon_switch_path=\"$(printf '%s\\n' \"$workon_output\" | sed -n 's/^__WORKON_SWITCH_PATH=//p' | tail -n 1)\"
  workon_switch_title=\"$(printf '%s\\n' \"$workon_output\" | sed -n 's/^__WORKON_SWITCH_TITLE=//p' | tail -n 1)\"
  if [ \"$workon_status\" -eq 0 ] && [ -n \"$workon_switch_path\" ]; then
    cd \"$workon_switch_path\" || return
    export {work_env}=\"$workon_switch_path\"
    export {title_env}=\"$workon_switch_title\"
  fi
  return \"$workon_status\"
}}

just() {{
  if [ \"${{1:-}}\" = \"wo\" ]; then
    shift
    wo \"$@\"
    return $?
  fi

  command just \"$@\"
}}
",
        active = WORKON_ACTIVE_ENV,
        root_env = WORKON_ROOT_ENV,
        work_env = WORKON_WORK_ENV,
        title_env = WORKON_TITLE_ENV,
        skip_user_zshrc = WORKON_SKIP_USER_ZSHRC_ENV,
        root = shell_quote(&root.display().to_string()),
        work = shell_quote(&work_path.display().to_string()),
        title = shell_quote(title),
        runner = runner
    ))
}

fn shell_runner() -> Result<String> {
    if let Ok(manifest_path) = std::env::var(WORKON_DEV_MANIFEST_ENV) {
        return Ok(format!(
            "cargo run --manifest-path {} -- --machine \"$@\"",
            shell_quote(&manifest_path)
        ));
    }

    let binary = std::env::current_exe()?;
    Ok(format!(
        "{} --machine \"$@\"",
        shell_quote(&binary.display().to_string())
    ))
}

fn zsh_path() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|shell| {
            Path::new(shell)
                .file_name()
                .is_some_and(|file_name| file_name == "zsh")
        })
        .unwrap_or_else(|| "zsh".to_string())
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn create(prefix: &str) -> Result<Self> {
        let base = std::env::temp_dir();
        for attempt in 0..1000 {
            let path = base.join(format!("{prefix}-{}-{attempt}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }

        Err(WorkonError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not create temporary Workon shell directory",
        )))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::shell_quote;

    #[test]
    fn quotes_shell_values_with_single_quotes() {
        assert_eq!(shell_quote("alpha'beta"), "'alpha'\\''beta'");
    }
}
