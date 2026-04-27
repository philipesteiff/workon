use std::path::PathBuf;

use crate::application::CommandOutput;
use crate::interfaces::shell::{install_shell_integration, ShellInstallKind};
use crate::shared::error::Result;

pub(crate) fn install_shell() -> Result<CommandOutput> {
    install(ShellInstallKind::Production, "shell integration installed")
}

pub(crate) fn install_dev_shell(manifest_path: PathBuf) -> Result<CommandOutput> {
    install(
        ShellInstallKind::Development { manifest_path },
        "dev shell integration installed",
    )
}

fn install(kind: ShellInstallKind, label: &str) -> Result<CommandOutput> {
    let outcome = install_shell_integration(kind)?;
    Ok(CommandOutput::ShellInstalled {
        label: label.to_string(),
        script_path: outcome.script_path,
        startup_path: outcome.startup_path,
    })
}
