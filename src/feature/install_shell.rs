use std::path::PathBuf;

use crate::app::CommandOutput;
use crate::error::Result;
use crate::shell_integration::{install_shell_integration, ShellInstallKind};

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
        zshrc_path: outcome.zshrc_path,
    })
}
