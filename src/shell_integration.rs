use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Result, WorkonError};

const WORKON_HOOK_ACTIVE_ENV: &str = "WORKON_HOOK_ACTIVE";
const WORKON_ROOT_ENV: &str = "WORKON_ROOT";
const WORKON_DEV_MANIFEST_ENV: &str = "WORKON_DEV_MANIFEST";

const PROD_SCRIPT: &str = "wo.zsh";
const DEV_SCRIPT: &str = "wo-dev.zsh";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShellInstallOutcome {
    pub script_path: PathBuf,
    pub zshrc_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShellInstallKind {
    Production,
    Development { manifest_path: PathBuf },
}

pub(crate) fn workon_root() -> std::io::Result<PathBuf> {
    root_from_env()
}

pub(crate) fn is_shell_hook_active() -> bool {
    std::env::var_os(WORKON_HOOK_ACTIVE_ENV).is_some()
}

pub(crate) fn install_shell_integration(kind: ShellInstallKind) -> Result<ShellInstallOutcome> {
    let home = home_dir()?;
    let script_dir = home.join(".workon/shell/zsh");
    fs::create_dir_all(&script_dir)?;

    let (script_name, script) = match kind {
        ShellInstallKind::Production => (PROD_SCRIPT, production_script()?),
        ShellInstallKind::Development { manifest_path } => {
            let repo_root = manifest_path
                .parent()
                .ok_or_else(|| WorkonError::MissingArgument {
                    message: "WORKON_DEV_MANIFEST must point to Cargo.toml".to_string(),
                })?
                .to_path_buf();
            (
                DEV_SCRIPT,
                render_zsh_integration(&dev_runner(&manifest_path), Some(&repo_root)),
            )
        }
    };

    let script_path = script_dir.join(script_name);
    fs::write(&script_path, script)?;

    let zshrc_path = home.join(".zshrc");
    ensure_source_line(&zshrc_path, &script_path)?;

    Ok(ShellInstallOutcome {
        script_path,
        zshrc_path,
    })
}

pub(crate) fn development_manifest_from_env() -> Result<PathBuf> {
    std::env::var(WORKON_DEV_MANIFEST_ENV)
        .map(PathBuf::from)
        .map_err(|_| WorkonError::MissingArgument {
            message: "WORKON_DEV_MANIFEST is required for install-dev-shell".to_string(),
        })
}

fn production_script() -> Result<String> {
    let binary = std::env::current_exe()?;
    Ok(render_zsh_integration(
        &shell_quote(&binary.display().to_string()),
        None,
    ))
}

fn dev_runner(manifest_path: &Path) -> String {
    format!(
        "cargo run --manifest-path {} --",
        shell_quote(&manifest_path.display().to_string())
    )
}

fn render_zsh_integration(runner: &str, dev_root: Option<&Path>) -> String {
    let dev_root_export = dev_root
        .map(|root| {
            format!(
                "export WORKON_DEV_ROOT={}\n",
                shell_quote(&root.display().to_string())
            )
        })
        .unwrap_or_default();
    let just_function = if dev_root.is_some() {
        "
just() {
  if [ \"${1:-}\" = \"wo\" ] && [ -n \"${WORKON_DEV_ROOT:-}\" ] && { [ \"$PWD\" = \"$WORKON_DEV_ROOT\" ] || [[ \"$PWD\" == \"$WORKON_DEV_ROOT\"/* ]]; }; then
    shift
    wo \"$@\"
    return $?
  fi

  command just \"$@\"
}
"
    } else {
        ""
    };

    format!(
        "\
# Workon shell integration.
{dev_root_export}
_workon_current_root() {{
  if [ -n \"${{WORKON_ROOT:-}}\" ]; then
    printf '%s\\n' \"$WORKON_ROOT\"
  else
    printf '%s\\n' \"$HOME\"
  fi
}}

_workon_run() {{
  local workon_output workon_status workon_cd workon_root workon_title workon_next_root workon_signal_file
  workon_root=\"$(_workon_current_root)\"

  if [ \"$#\" -eq 0 ]; then
    workon_signal_file=\"$(mktemp \"${{TMPDIR:-/tmp}}/workon-signals.XXXXXX\")\" || return
    {hook_active}=1 {root_env}=\"$workon_root\" WORKON_SIGNAL_FILE=\"$workon_signal_file\" {runner} \"$@\"
    workon_status=$?
    workon_output=\"$(cat \"$workon_signal_file\" 2>/dev/null)\"
    rm -f \"$workon_signal_file\"
  else
    workon_output=\"$({hook_active}=1 {root_env}=\"$workon_root\" {runner} --machine \"$@\" 2>&1)\"
    workon_status=$?
    printf '%s\\n' \"$workon_output\" | sed '/^__WORKON_/d'
  fi

  workon_cd=\"$(printf '%s\\n' \"$workon_output\" | sed -n 's/^__WORKON_CD=//p' | tail -n 1)\"
  workon_next_root=\"$(printf '%s\\n' \"$workon_output\" | sed -n 's/^__WORKON_ROOT=//p' | tail -n 1)\"
  workon_title=\"$(printf '%s\\n' \"$workon_output\" | sed -n 's/^__WORKON_TITLE=//p' | tail -n 1)\"

  if [ \"$workon_status\" -eq 0 ] && [ -n \"$workon_cd\" ]; then
    cd \"$workon_cd\" || return
    export WORKON_ROOT=\"${{workon_next_root:-$workon_root}}\"
    export WORKON_WORK=\"$workon_cd\"
    export WORKON_TITLE=\"$workon_title\"
  fi

  return \"$workon_status\"
}}

wo() {{
  _workon_run \"$@\"
}}
{just_function}",
        hook_active = WORKON_HOOK_ACTIVE_ENV,
        root_env = WORKON_ROOT_ENV,
    )
}

fn ensure_source_line(zshrc_path: &Path, script_path: &Path) -> Result<()> {
    let source_line = format!(
        "[ -f {script} ] && source {script}",
        script = shell_quote(&script_path.display().to_string())
    );

    let existing = match fs::read_to_string(zshrc_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };

    if existing.contains(&source_line) {
        return Ok(());
    }

    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str("\n# Workon shell integration\n");
    next.push_str(&source_line);
    next.push('\n');

    fs::write(zshrc_path, next)?;
    Ok(())
}

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| WorkonError::MissingArgument {
            message: "HOME is required to install shell integration".to_string(),
        })
}

fn root_from_env() -> io::Result<PathBuf> {
    if let Some(root) = non_empty_env(WORKON_ROOT_ENV) {
        return Ok(PathBuf::from(root));
    }

    non_empty_env("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is required"))
}

fn non_empty_env(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::{ensure_source_line, render_zsh_integration, shell_quote};
    use std::fs;
    use std::path::Path;

    #[test]
    fn quotes_shell_values_with_single_quotes() {
        assert_eq!(shell_quote("alpha'beta"), "'alpha'\\''beta'");
    }

    #[test]
    fn generated_script_defines_workon_function_and_cd_signal() {
        let script = render_zsh_integration("'wo'", None);

        assert!(script.contains("wo()"));
        assert!(script.contains("__WORKON_CD="));
        assert!(script.contains("cd \"$workon_cd\""));
        assert!(script.contains("WORKON_SIGNAL_FILE=\"$workon_signal_file\" 'wo' \"$@\""));
        assert!(script.contains("'wo' --machine \"$@\""));
        assert!(!script.contains("just()"));
    }

    #[test]
    fn generated_dev_script_intercepts_just_wo() {
        let script = render_zsh_integration(
            "cargo run --manifest-path '/repo/Cargo.toml' --",
            Some(Path::new("/repo")),
        );

        assert!(script.contains("export WORKON_DEV_ROOT='/repo'"));
        assert!(script.contains("just()"));
        assert!(script.contains("wo \"$@\""));
        assert!(script.contains("cargo run --manifest-path '/repo/Cargo.toml' -- \"$@\""));
        assert!(script.contains("cargo run --manifest-path '/repo/Cargo.toml' -- --machine \"$@\""));
    }

    #[test]
    fn source_line_is_idempotent() {
        let path = std::env::temp_dir().join(format!("workon-zshrc-test-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let script = Path::new("/tmp/workon-test/wo.zsh");

        ensure_source_line(&path, script).expect("first install should work");
        ensure_source_line(&path, script).expect("second install should work");

        let content = fs::read_to_string(&path).expect("zshrc should be readable");
        assert_eq!(content.matches("Workon shell integration").count(), 1);

        let _ = fs::remove_file(path);
    }
}
