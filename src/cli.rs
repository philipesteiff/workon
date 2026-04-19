use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use crate::app::{App, Command, CommandOutput};
use crate::error::{Result, WorkonError};

const WORKON_ACTIVE_ENV: &str = "WORKON_ACTIVE";
const WORKON_ROOT_ENV: &str = "WORKON_ROOT";
const WORKON_WORK_ENV: &str = "WORKON_WORK";
const WORKON_TITLE_ENV: &str = "WORKON_TITLE";
const WORKON_DEV_MANIFEST_ENV: &str = "WORKON_DEV_MANIFEST";
const WORKON_SHELL_COMMAND_ENV: &str = "WORKON_SHELL_COMMAND";
const WORKON_SKIP_USER_ZSHRC_ENV: &str = "WORKON_SKIP_USER_ZSHRC";

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

    if machine {
        return Ok(0);
    }

    match &output {
        CommandOutput::WorkCreated(work) => enter_work_shell(&root, &work.title, &work.path),
        CommandOutput::WorkOpened(work) => enter_work_shell(&root, &work.title, &work.path),
        CommandOutput::Context(_) | CommandOutput::WorkList(_) => Ok(0),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliRequest {
    Command { command: Command, machine: bool },
    Help,
}

fn parse_args(args: Vec<String>) -> Result<CliRequest> {
    if args.is_empty() {
        return Ok(command_request(Command::ListWorks, false));
    }

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(CliRequest::Help);
    }

    let mut machine = false;
    let mut intent_id = None;
    let mut input_parts = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--machine" => {
                machine = true;
                index += 1;
            }
            "--intent" | "-i" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(WorkonError::MissingArgument {
                        message: "--intent requires an intent id".to_string(),
                    });
                };
                intent_id = Some(value.clone());
                index += 2;
            }
            value => {
                input_parts.push(value.to_string());
                index += 1;
            }
        }
    }

    let input = input_parts.join(" ");
    if input.trim().is_empty() && intent_id.is_none() {
        return Ok(command_request(Command::ListWorks, machine));
    }

    if input == "ctx" || input == "context" {
        return Ok(command_request(Command::Context, machine));
    }

    if input.trim().is_empty() {
        return Err(WorkonError::MissingArgument {
            message: "missing work goal or work query".to_string(),
        });
    }

    Ok(command_request(
        Command::OpenOrCreate { input, intent_id },
        machine,
    ))
}

fn command_request(command: Command, machine: bool) -> CliRequest {
    CliRequest::Command { command, machine }
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

fn workon_root() -> std::io::Result<PathBuf> {
    match std::env::var(WORKON_ROOT_ENV) {
        Ok(root) => Ok(PathBuf::from(root)),
        Err(_) => std::env::current_dir(),
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

fn render_output(output: &CommandOutput, writer: &mut dyn Write, machine: bool) -> Result<()> {
    match output {
        CommandOutput::Context(context) => {
            writeln!(writer, "context: {}", context.status)?;
            writeln!(writer, "{}", context.message)?;
        }
        CommandOutput::WorkCreated(work) => {
            writeln!(writer, "intent selected: {}", work.intent_id)?;
            writeln!(writer, "work created: {}", work.title)?;
            writeln!(writer, "folder created: {}", work.path.display())?;
            writeln!(writer, "created: AGENTS.md")?;
            writeln!(writer, "created: CLAUDE.md")?;
            render_switch_signal(writer, &work.path, &work.title, machine)?;
        }
        CommandOutput::WorkList(list) => {
            if list.works.is_empty() {
                writeln!(writer, "no work yet")?;
            } else {
                for work in &list.works {
                    writeln!(writer, "{}\t{}", work.slug, work.title)?;
                }
            }
        }
        CommandOutput::WorkOpened(work) => {
            writeln!(writer, "work opened: {}", work.title)?;
            writeln!(writer, "path: {}", work.path.display())?;
            render_switch_signal(writer, &work.path, &work.title, machine)?;
        }
    }
    Ok(())
}

fn render_switch_signal(
    writer: &mut dyn Write,
    path: &Path,
    title: &str,
    machine: bool,
) -> Result<()> {
    if machine {
        writeln!(writer, "__WORKON_SWITCH_PATH={}", path.display())?;
        writeln!(writer, "__WORKON_SWITCH_TITLE={title}")?;
    }
    Ok(())
}

fn enter_work_shell(root: &Path, title: &str, work_path: &Path) -> Result<i32> {
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

fn write_help(writer: &mut dyn Write) -> Result<()> {
    writeln!(
        writer,
        "wo\n\
         wo <work-query>\n\
         wo --intent <intent-id> \"<goal>\"\n\
         wo ctx"
    )?;
    Ok(())
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
