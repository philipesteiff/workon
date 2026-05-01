use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use crate::application::{App, Command, CommandOutput};
use crate::interfaces::shell::{is_shell_hook_active, workon_root};
use crate::shared::error::{Result, WorkonError};

mod args;
mod output;

use self::args::{parse_args, CliRequest};
use self::output::{render_output, write_help, write_intent_help, write_repos_help, write_version};

type TuiRunner = fn(&App, PathBuf) -> Result<Option<CommandOutput>>;

pub fn run_cli(args: impl IntoIterator<Item = String>) -> i32 {
    let root = match workon_root() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("error: {error}");
            return 1;
        }
    };

    let mut stdout = io::stdout();
    let mut stderr = io::stderr();

    match run(args, root, &mut stdout, &mut stderr) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            let _ = writeln!(stderr, "error: {error}");
            1
        }
    }
}

fn run(
    args: impl IntoIterator<Item = String>,
    root: PathBuf,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<i32> {
    let app = App::new(root.clone());
    let request = parse_args(args.into_iter().collect())?;

    if let CliRequest::Help = request {
        write_help(stdout)?;
        return Ok(0);
    }

    if let CliRequest::ReposHelp = request {
        write_repos_help(stdout)?;
        return Ok(0);
    }

    if let CliRequest::IntentHelp = request {
        write_intent_help(stdout)?;
        return Ok(0);
    }

    if let CliRequest::Version = request {
        write_version(stdout)?;
        return Ok(0);
    }

    let CliRequest::Command {
        command,
        machine,
        allow_tui,
    } = request
    else {
        unreachable!("help request returned earlier");
    };

    if should_run_tui(&command, machine, allow_tui) {
        return run_tui_or_render_list(&app, &root, stdout, stderr, crate::interfaces::tui::run);
    }

    if should_offer_shell_install(&command, machine) {
        return offer_shell_install(&app, &root, stdout, stderr);
    }

    let output = execute_command(&app, command, stdout, stderr)?;
    render_output(&output, stdout, machine, &root)?;
    Ok(0)
}

fn run_tui_or_render_list(
    app: &App,
    root: &std::path::Path,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    tui_runner: TuiRunner,
) -> Result<i32> {
    match tui_runner(app, root.to_path_buf()) {
        Ok(Some(output)) => {
            render_output(&output, stdout, false, root)?;
        }
        Ok(None) => {}
        Err(WorkonError::TuiUnavailable { message }) => {
            writeln!(
                stderr,
                "warning: TUI unavailable ({message}); showing list instead."
            )?;
            let output = app.execute(Command::ListWorks)?;
            render_output(&output, stdout, false, root)?;
        }
        Err(error) => return Err(error),
    }

    Ok(0)
}

fn execute_command(
    app: &App,
    command: Command,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<CommandOutput> {
    match app.execute(command.clone()) {
        Ok(output) => Ok(output),
        Err(WorkonError::IntentRequired { .. }) => {
            let intent_id = prompt_for_intent(app, stdout, stderr)?;
            let Command::OpenOrCreate { input, .. } = command else {
                return Err(WorkonError::IntentRequired {
                    available: app
                        .available_intents()
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect(),
                });
            };
            app.execute(Command::OpenOrCreate {
                input,
                intent_id: Some(intent_id),
            })
        }
        Err(error) => Err(error),
    }
}

fn should_offer_shell_install(command: &Command, machine: bool) -> bool {
    !machine
        && !is_shell_hook_active()
        && io::stdin().is_terminal()
        && matches!(
            command,
            Command::CreateWork { .. } | Command::OpenOrCreate { .. } | Command::OpenWork { .. }
        )
}

fn should_run_tui(command: &Command, machine: bool, allow_tui: bool) -> bool {
    allow_tui
        && !machine
        && io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && matches!(command, Command::ListWorks)
}

fn offer_shell_install(
    app: &App,
    root: &std::path::Path,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<i32> {
    writeln!(
        stdout,
        "Workon needs a shell function to switch folders in your current shell."
    )?;
    writeln!(stdout, "Install it now? [y/N]")?;
    write!(stdout, "> ")?;
    stdout.flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    if answer != "y" && answer != "yes" {
        writeln!(stderr, "shell integration not installed; command not run")?;
        return Ok(1);
    }

    let command = shell_install_command_for_prompt(
        std::env::var_os("WORKON_DEV_MANIFEST").map(PathBuf::from),
    );
    let output = app.execute(command)?;
    render_output(&output, stdout, false, root)?;
    Ok(0)
}

fn shell_install_command_for_prompt(dev_manifest: Option<PathBuf>) -> Command {
    match dev_manifest {
        Some(manifest_path) => Command::InstallDevShell { manifest_path },
        None => Command::InstallShell,
    }
}

fn prompt_for_intent(app: &App, stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<String> {
    if !io::stdin().is_terminal() {
        return Err(WorkonError::IntentRequired {
            available: app
                .available_intents()
                .into_iter()
                .map(|(id, _)| id)
                .collect(),
        });
    }

    writeln!(stdout, "select intent:")?;
    let intents = app.available_intents();
    for (index, (id, summary)) in intents.iter().enumerate() {
        writeln!(stdout, "{}. {} - {}", index + 1, id, summary)?;
    }
    write!(stdout, "> ")?;
    stdout.flush()?;

    let mut selected = String::new();
    io::stdin().read_line(&mut selected)?;
    let selected = selected.trim();

    if let Ok(number) = selected.parse::<usize>() {
        if let Some((id, _)) = intents.get(number.saturating_sub(1)) {
            return Ok(id.clone());
        }
    }

    if intents.iter().any(|(id, _)| id == selected) {
        return Ok(selected.to_string());
    }

    writeln!(stderr, "invalid intent selection: {selected}")?;
    Err(WorkonError::UnknownIntent {
        intent_id: selected.to_string(),
        available: intents.into_iter().map(|(id, _)| id).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{run_tui_or_render_list, shell_install_command_for_prompt};
    use crate::application::Command;
    use crate::shared::error::{Result, WorkonError};
    use crate::{App, CommandOutput};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn prompt_install_uses_development_hook_when_dev_manifest_is_present() {
        let manifest = PathBuf::from("/repo/Cargo.toml");

        assert_eq!(
            shell_install_command_for_prompt(Some(manifest.clone())),
            Command::InstallDevShell {
                manifest_path: manifest
            }
        );
    }

    #[test]
    fn prompt_install_uses_production_hook_without_dev_manifest() {
        assert_eq!(
            shell_install_command_for_prompt(None),
            Command::InstallShell
        );
    }

    #[test]
    fn tui_startup_unavailable_falls_back_to_cli_list() {
        let root = temp_root("tui_startup_unavailable_falls_back_to_cli_list");
        let app = App::new(root.path.clone());
        app.execute(Command::CreateWork {
            goal: "Investigate payment retry latency".to_string(),
            intent_id: "investigate".to_string(),
        })
        .expect("work should create");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = run_tui_or_render_list(
            &app,
            &root.path,
            &mut stdout,
            &mut stderr,
            fake_unavailable_tui,
        )
        .expect("fallback should render list");

        assert_eq!(exit_code, 0);
        let stdout = String::from_utf8(stdout).expect("stdout should be utf8");
        let stderr = String::from_utf8(stderr).expect("stderr should be utf8");
        assert!(stdout.contains("Investigate payment retry latency"));
        assert!(stdout.contains("investigate-payment-retry-latency"));
        assert!(stderr.contains("warning: TUI unavailable"));
        assert!(stderr.contains("cursor position could not be read"));
    }

    fn fake_unavailable_tui(_app: &App, _root: PathBuf) -> Result<Option<CommandOutput>> {
        Err(WorkonError::TuiUnavailable {
            message: "cursor position could not be read".to_string(),
        })
    }

    struct TempRoot {
        path: PathBuf,
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn temp_root(name: &str) -> TempRoot {
        let mut path = std::env::temp_dir();
        path.push(format!("workon-cli-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp root should be created");
        TempRoot { path }
    }
}
