use crate::app::Command;
use crate::error::{Result, WorkonError};
use crate::shell_integration::development_manifest_from_env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliRequest {
    Command {
        command: Command,
        machine: bool,
        allow_tui: bool,
    },
    Help,
}

pub(crate) fn parse_args(args: Vec<String>) -> Result<CliRequest> {
    if args.is_empty() {
        return Ok(list_request(false, true));
    }

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(CliRequest::Help);
    }

    let lone_option_terminator = args.len() == 1 && args[0] == "--";
    let command_args = args
        .iter()
        .filter(|arg| arg.as_str() != "--machine")
        .cloned()
        .collect::<Vec<_>>();

    if command_args == ["install-shell"] {
        return Ok(command_request(Command::InstallShell, false));
    }

    if command_args == ["install-dev-shell"] {
        return Ok(command_request(
            Command::InstallDevShell {
                manifest_path: development_manifest_from_env()?,
            },
            false,
        ));
    }

    let mut machine = false;
    let mut intent_id = None;
    let mut input_parts = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--" => {
                input_parts.extend(args[(index + 1)..].iter().cloned());
                break;
            }
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
        return Ok(list_request(machine, lone_option_terminator && !machine));
    }

    if input == "list" {
        return Ok(list_request(machine, false));
    }

    if let Some(query) = input.strip_prefix("archive ") {
        let query = query.trim();
        if query.is_empty() {
            return Err(WorkonError::MissingArgument {
                message: "archive requires a work query".to_string(),
            });
        }

        return Ok(command_request(
            Command::ArchiveWork {
                query: query.to_string(),
            },
            machine,
        ));
    }

    if input == "archive" {
        return Err(WorkonError::MissingArgument {
            message: "archive requires a work query".to_string(),
        });
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
    CliRequest::Command {
        command,
        machine,
        allow_tui: false,
    }
}

fn list_request(machine: bool, allow_tui: bool) -> CliRequest {
    CliRequest::Command {
        command: Command::ListWorks,
        machine,
        allow_tui,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_args, CliRequest};
    use crate::app::Command;

    #[test]
    fn empty_args_allow_tui_list() {
        let request = parse_args(Vec::new()).expect("args should parse");

        assert_eq!(
            request,
            CliRequest::Command {
                command: Command::ListWorks,
                machine: false,
                allow_tui: true,
            }
        );
    }

    #[test]
    fn treats_double_dash_as_option_terminator() {
        let request = parse_args(vec![
            "--".to_string(),
            "answer-technical-question-manager-2".to_string(),
        ])
        .expect("args should parse");

        assert_eq!(
            request,
            CliRequest::Command {
                command: Command::OpenOrCreate {
                    input: "answer-technical-question-manager-2".to_string(),
                    intent_id: None,
                },
                machine: false,
                allow_tui: false,
            }
        );
    }

    #[test]
    fn treats_lone_double_dash_as_empty_args() {
        let request = parse_args(vec!["--".to_string()]).expect("args should parse");

        assert_eq!(
            request,
            CliRequest::Command {
                command: Command::ListWorks,
                machine: false,
                allow_tui: true,
            }
        );
    }

    #[test]
    fn parses_shell_install_commands() {
        assert_eq!(
            parse_args(vec!["install-shell".to_string()]).expect("args should parse"),
            CliRequest::Command {
                command: Command::InstallShell,
                machine: false,
                allow_tui: false,
            }
        );

        let previous = std::env::var_os("WORKON_DEV_MANIFEST");
        std::env::set_var("WORKON_DEV_MANIFEST", "/repo/Cargo.toml");
        assert_eq!(
            parse_args(vec!["install-dev-shell".to_string()]).expect("args should parse"),
            CliRequest::Command {
                command: Command::InstallDevShell {
                    manifest_path: "/repo/Cargo.toml".into(),
                },
                machine: false,
                allow_tui: false,
            }
        );
        restore_env("WORKON_DEV_MANIFEST", previous);

        assert_eq!(
            parse_args(vec!["--machine".to_string(), "install-shell".to_string()])
                .expect("args should parse"),
            CliRequest::Command {
                command: Command::InstallShell,
                machine: false,
                allow_tui: false,
            }
        );
    }

    #[test]
    fn parses_archive_command() {
        assert_eq!(
            parse_args(vec!["archive".to_string(), "billing".to_string()])
                .expect("args should parse"),
            CliRequest::Command {
                command: Command::ArchiveWork {
                    query: "billing".to_string(),
                },
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec![
                "--machine".to_string(),
                "archive".to_string(),
                "billing".to_string()
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::ArchiveWork {
                    query: "billing".to_string(),
                },
                machine: true,
                allow_tui: false,
            }
        );
    }

    #[test]
    fn archive_requires_query() {
        let error =
            parse_args(vec!["archive".to_string()]).expect_err("archive without query should fail");

        assert_eq!(error.to_string(), "archive requires a work query");
    }

    #[test]
    fn parses_explicit_list_command() {
        assert_eq!(
            parse_args(vec!["list".to_string()]).expect("args should parse"),
            CliRequest::Command {
                command: Command::ListWorks,
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec!["--machine".to_string(), "list".to_string()])
                .expect("args should parse"),
            CliRequest::Command {
                command: Command::ListWorks,
                machine: true,
                allow_tui: false,
            }
        );
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
