use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use crate::app::{App, Command, CommandOutput};
use crate::cli_args::{parse_args, CliRequest};
use crate::cli_output::{render_output, write_help};
use crate::error::{Result, WorkonError};
use crate::work_shell::{enter_work_shell, is_work_shell_active, workon_root};

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

    let CliRequest::Command { command, machine } = request else {
        unreachable!("help request returned earlier");
    };

    let output = execute_command(&app, command, stdout, stderr)?;
    render_output(&output, stdout, machine)?;

    if machine || is_work_shell_active() {
        return Ok(0);
    }

    match &output {
        CommandOutput::WorkCreated(work) => enter_work_shell(&root, &work.title, &work.path),
        CommandOutput::WorkOpened(work) => enter_work_shell(&root, &work.title, &work.path),
        CommandOutput::Context(_) | CommandOutput::WorkList(_) => Ok(0),
    }
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
