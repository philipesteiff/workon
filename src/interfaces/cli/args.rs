use crate::application::Command;
use crate::interfaces::shell::development_manifest_from_env;
use crate::shared::error::{Result, WorkonError};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliRequest {
    Command {
        command: Command,
        machine: bool,
        allow_tui: bool,
    },
    Help,
    IntentHelp,
    ReposHelp,
    Version,
}

pub(crate) fn parse_args(args: Vec<String>) -> Result<CliRequest> {
    if args.is_empty() {
        return Ok(list_request(false, true));
    }

    if is_repos_help_request(&args) {
        return Ok(CliRequest::ReposHelp);
    }

    if is_intent_help_request(&args) {
        return Ok(CliRequest::IntentHelp);
    }

    let args_before_terminator = args
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .filter(|arg| arg.as_str() != "--machine")
        .cloned()
        .collect::<Vec<_>>();

    if args_before_terminator
        .iter()
        .any(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(match args_before_terminator.first().map(String::as_str) {
            Some("repos") => CliRequest::ReposHelp,
            Some("intent") => CliRequest::IntentHelp,
            _ => CliRequest::Help,
        });
    }

    let lone_option_terminator = args.len() == 1 && args[0] == "--";
    let command_args = args
        .iter()
        .filter(|arg| arg.as_str() != "--machine")
        .cloned()
        .collect::<Vec<_>>();

    if let Some(request) = parse_help_command(&command_args)? {
        return Ok(request);
    }

    if is_version_request(&command_args) {
        return Ok(CliRequest::Version);
    }

    reject_reserved_top_level_extras(&command_args, "version")?;
    reject_reserved_top_level_extras(&command_args, "install-shell")?;
    reject_reserved_top_level_extras(&command_args, "install-dev-shell")?;

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
    let mut option_terminator_used = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--" => {
                if is_repos_command_local_terminator(&input_parts) {
                    index += 1;
                    continue;
                }
                option_terminator_used = true;
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
            value if input_parts.is_empty() && value.starts_with('-') => {
                return Err(WorkonError::MissingArgument {
                    message: format!(
                        "unknown option `{value}`. Use `--` before a work goal that starts with `-`."
                    ),
                });
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

    if input.trim().is_empty() {
        let intent_id = intent_id.expect("empty input with no intent handled above");
        return Err(WorkonError::MissingArgument {
            message: format!(
                "--intent requires a work goal. Use `wo --intent {intent_id} \"<goal>\"`."
            ),
        });
    }

    if option_terminator_used && !input.trim().is_empty() {
        return Ok(command_request(
            Command::OpenOrCreate { input, intent_id },
            machine,
        ));
    }

    if intent_id.is_some()
        && !option_terminator_used
        && is_reserved_command_invocation(&input_parts)
    {
        return Err(WorkonError::MissingArgument {
            message: "--intent can only be used when creating or opening Work. Use `--` before a Work goal that starts with a command name.".to_string(),
        });
    }

    if input_parts == ["repos"] {
        return Ok(CliRequest::ReposHelp);
    }

    if input_parts == ["intent"] {
        return Ok(CliRequest::IntentHelp);
    }

    if input_parts.first().is_some_and(|part| part == "repos") {
        return parse_repos_command(&input_parts, machine);
    }

    if input_parts.first().is_some_and(|part| part == "intent") {
        return parse_intent_command(&input_parts, machine);
    }

    if input_parts.first().is_some_and(|part| part == "list") && input_parts.len() > 1 {
        return Err(WorkonError::MissingArgument {
            message: "list does not accept extra arguments. Use `wo \"list ...\"` to create a Work goal that starts with list.".to_string(),
        });
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

fn is_repos_command_local_terminator(input_parts: &[String]) -> bool {
    matches!(
        (
            input_parts.first().map(String::as_str),
            input_parts.get(1).map(String::as_str),
        ),
        (Some("repos"), Some("add" | "link" | "remove"))
    )
}

fn is_repos_help_request(args: &[String]) -> bool {
    matches!(
        args,
        [command] if command == "repos"
    ) || matches!(
        args,
        [command, help] if command == "repos" && (help == "--help" || help == "-h" || help == "help")
    ) || matches!(
        args,
        [help, topic] if help == "help" && topic == "repos"
    )
}

fn is_intent_help_request(args: &[String]) -> bool {
    matches!(
        args,
        [command] if command == "intent"
    ) || matches!(
        args,
        [command, help] if command == "intent" && (help == "--help" || help == "-h" || help == "help")
    ) || matches!(
        args,
        [help, topic] if help == "help" && topic == "intent"
    )
}

fn is_version_request(args: &[String]) -> bool {
    matches!(
        args,
        [command] if command == "version" || command == "--version" || command == "-V"
    )
}

fn parse_help_command(args: &[String]) -> Result<Option<CliRequest>> {
    if args.first().is_none_or(|part| part != "help") {
        return Ok(None);
    }

    match args {
        [_] => Ok(Some(CliRequest::Help)),
        [_, topic] if topic == "repos" => Ok(Some(CliRequest::ReposHelp)),
        [_, topic] if topic == "intent" => Ok(Some(CliRequest::IntentHelp)),
        [_, topic, ..] if topic == "repos" || topic == "intent" => {
            Err(WorkonError::MissingArgument {
                message: format!("help {topic} does not accept extra arguments"),
            })
        }
        [_, topic, ..] => Err(WorkonError::MissingArgument {
            message: format!(
                "unknown help topic `{topic}`. Use `wo help`, `wo help repos`, or `wo help intent`."
            ),
        }),
        [] => Ok(None),
    }
}

fn reject_reserved_top_level_extras(args: &[String], command: &str) -> Result<()> {
    if args.first().is_some_and(|part| part == command) && args.len() > 1 {
        return Err(WorkonError::MissingArgument {
            message: format!(
                "{command} does not accept extra arguments. Use `wo \"{command} ...\"` to create a Work goal that starts with {command}."
            ),
        });
    }

    Ok(())
}

fn is_reserved_command_invocation(input_parts: &[String]) -> bool {
    input_parts.first().is_some_and(|part| {
        matches!(
            part.as_str(),
            "archive"
                | "help"
                | "install-dev-shell"
                | "install-shell"
                | "intent"
                | "list"
                | "repos"
                | "version"
        )
    })
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

fn parse_repos_command(input_parts: &[String], machine: bool) -> Result<CliRequest> {
    match input_parts.get(1).map(String::as_str) {
        Some("help") => {
            reject_extra_arguments(input_parts, 2, "repos help does not accept extra arguments")?;
            Ok(CliRequest::ReposHelp)
        }
        Some("workspace") | Some("workspaces") => {
            parse_repos_workspace_command(input_parts, machine)
        }
        Some("discover") => {
            reject_unknown_leading_option(input_parts.get(2), "repos discover")?;
            let query = input_parts[2..].join(" ");
            if query.trim().is_empty() {
                return Err(WorkonError::MissingArgument {
                    message: "repos discover requires a work query".to_string(),
                });
            }
            Ok(command_request(
                Command::ListRepositoryCandidates { query },
                machine,
            ))
        }
        Some("candidate-paths") => {
            reject_unknown_leading_option(input_parts.get(2), "repos candidate-paths")?;
            let query = input_parts[2..].join(" ");
            if query.trim().is_empty() {
                return Err(WorkonError::MissingArgument {
                    message: "repos candidate-paths requires a work query".to_string(),
                });
            }
            Ok(command_request(
                Command::ListRepositoryCandidatePaths { query },
                machine,
            ))
        }
        Some("link") => {
            reject_unknown_leading_option(input_parts.get(2), "repos link")?;
            let (query, paths) = parse_work_query_and_paths(&input_parts[2..], "repos link")?;
            Ok(command_request(
                Command::LinkWorkRepositories { query, paths },
                machine,
            ))
        }
        Some("list") => {
            reject_unknown_leading_option(input_parts.get(2), "repos list")?;
            let query = input_parts[2..].join(" ");
            if query.trim().is_empty() {
                return Err(WorkonError::MissingArgument {
                    message: "repos list requires a work query".to_string(),
                });
            }
            Ok(command_request(
                Command::ListWorkRepositories { query },
                machine,
            ))
        }
        Some("add") => {
            let (workspace, positional) = parse_workspace_option(&input_parts[2..])?;
            let (query, repositories) =
                parse_work_query_and_repositories(&positional, "repos add")?;
            Ok(command_request(
                Command::AddWorkRepositories {
                    query,
                    repositories,
                    workspace,
                },
                machine,
            ))
        }
        Some("remove") => {
            reject_unknown_options(&input_parts[2..], &["--force"], "repos remove")?;
            let force = input_parts[2..].iter().any(|part| part == "--force");
            let parts = input_parts[2..]
                .iter()
                .filter(|part| part.as_str() != "--force")
                .cloned()
                .collect::<Vec<_>>();
            let (query, repositories) = parse_work_query_and_repositories(&parts, "repos remove")?;
            Ok(command_request(
                Command::RemoveWorkRepositories {
                    query,
                    repositories,
                    force,
                },
                machine,
            ))
        }
        _ => Err(WorkonError::MissingArgument {
            message:
                "repos requires list, add, link, discover, candidate-paths, workspace, or remove"
                    .to_string(),
        }),
    }
}

fn parse_repos_workspace_command(input_parts: &[String], machine: bool) -> Result<CliRequest> {
    match input_parts.get(2).map(String::as_str) {
        Some("list") => {
            reject_extra_arguments(
                input_parts,
                3,
                "repos workspace list does not accept extra arguments",
            )?;
            Ok(command_request(Command::ListRepositoryWorkspaces, machine))
        }
        None => Ok(command_request(Command::ListRepositoryWorkspaces, machine)),
        Some("add") => {
            let paths = input_parts
                .get(3..)
                .unwrap_or_default()
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>();
            if paths.is_empty() {
                return Err(WorkonError::MissingArgument {
                    message: "repos workspace add requires at least one path".to_string(),
                });
            }
            Ok(command_request(
                Command::AddRepositoryWorkspaces { paths },
                machine,
            ))
        }
        Some("remove") => {
            let Some(path) = input_parts.get(3) else {
                return Err(WorkonError::MissingArgument {
                    message: "repos workspace remove requires a path".to_string(),
                });
            };
            reject_extra_arguments(
                input_parts,
                4,
                "repos workspace remove accepts exactly one path",
            )?;
            Ok(command_request(
                Command::RemoveRepositoryWorkspace {
                    path: PathBuf::from(path),
                },
                machine,
            ))
        }
        _ => Err(WorkonError::MissingArgument {
            message: "repos workspace requires list, add, or remove".to_string(),
        }),
    }
}

fn parse_intent_command(input_parts: &[String], machine: bool) -> Result<CliRequest> {
    match input_parts.get(1).map(String::as_str) {
        Some("help") => {
            reject_extra_arguments(
                input_parts,
                2,
                "intent help does not accept extra arguments",
            )?;
            Ok(CliRequest::IntentHelp)
        }
        Some("list") => {
            reject_extra_arguments(
                input_parts,
                2,
                "intent list does not accept extra arguments",
            )?;
            Ok(command_request(Command::ListIntents, machine))
        }
        Some("show") => {
            let Some(intent_id) = input_parts.get(2) else {
                return Err(WorkonError::MissingArgument {
                    message: "intent show requires an intent id".to_string(),
                });
            };
            reject_extra_arguments(input_parts, 3, "intent show accepts exactly one intent id")?;
            Ok(command_request(
                Command::ShowIntent {
                    intent_id: intent_id.clone(),
                },
                machine,
            ))
        }
        Some("switch") => {
            if input_parts.len() < 4 {
                return Err(WorkonError::MissingArgument {
                    message: "intent switch requires a work query and intent id".to_string(),
                });
            };
            let intent_id = input_parts.last().expect("intent id checked above");
            let query = input_parts[2..input_parts.len() - 1].join(" ");
            Ok(command_request(
                Command::SwitchWorkIntent {
                    query,
                    intent_id: intent_id.clone(),
                },
                machine,
            ))
        }
        _ => Err(WorkonError::MissingArgument {
            message: "intent requires list, show, or switch".to_string(),
        }),
    }
}

fn reject_extra_arguments(
    input_parts: &[String],
    expected_len: usize,
    message: &str,
) -> Result<()> {
    if input_parts.len() > expected_len {
        return Err(WorkonError::MissingArgument {
            message: message.to_string(),
        });
    }

    Ok(())
}

fn parse_workspace_option(parts: &[String]) -> Result<(Option<PathBuf>, Vec<String>)> {
    let mut workspace = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < parts.len() {
        if parts[index] == "--workspace" {
            let Some(value) = parts.get(index + 1) else {
                return Err(WorkonError::MissingArgument {
                    message: "--workspace requires a path".to_string(),
                });
            };
            workspace = Some(PathBuf::from(value));
            index += 2;
        } else if parts[index].starts_with('-') {
            return Err(WorkonError::MissingArgument {
                message: format!(
                    "unknown repos add option `{}`. Use `--workspace <path>` before the work query.",
                    parts[index]
                ),
            });
        } else {
            positional.push(parts[index].clone());
            index += 1;
        }
    }
    Ok((workspace, positional))
}

fn parse_work_query_and_repositories(
    positional: &[String],
    command: &str,
) -> Result<(String, Vec<String>)> {
    if positional.is_empty() {
        return Err(WorkonError::MissingArgument {
            message: format!("{command} requires a work query and at least one repository"),
        });
    }

    let Some(repository_start) = positional
        .iter()
        .position(|part| looks_like_repository(part))
    else {
        let Some(query) = positional.first() else {
            unreachable!("empty positional handled above");
        };
        let repositories = positional[1..].to_vec();
        if repositories.is_empty() {
            return Err(WorkonError::MissingArgument {
                message: format!("{command} requires at least one repository"),
            });
        }
        return Ok((query.clone(), repositories));
    };

    if repository_start == 0 {
        return Err(WorkonError::MissingArgument {
            message: format!("{command} requires a work query and at least one repository"),
        });
    }

    Ok((
        positional[..repository_start].join(" "),
        positional[repository_start..].to_vec(),
    ))
}

fn parse_work_query_and_paths(
    positional: &[String],
    command: &str,
) -> Result<(String, Vec<PathBuf>)> {
    if positional.is_empty() {
        return Err(WorkonError::MissingArgument {
            message: format!("{command} requires a work query and at least one path"),
        });
    }

    let Some(path_start) = positional.iter().position(|part| looks_like_path(part)) else {
        let Some(query) = positional.first() else {
            unreachable!("empty positional handled above");
        };
        let paths = positional[1..]
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if paths.is_empty() {
            return Err(WorkonError::MissingArgument {
                message: format!("{command} requires at least one path"),
            });
        }
        return Ok((query.clone(), paths));
    };

    if path_start == 0 {
        return Err(WorkonError::MissingArgument {
            message: format!("{command} requires a work query and at least one path"),
        });
    }

    Ok((
        positional[..path_start].join(" "),
        positional[path_start..]
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>(),
    ))
}

fn looks_like_repository(value: &str) -> bool {
    let mut parts = value.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(repo), None) if !owner.is_empty() && !repo.is_empty()
    )
}

fn looks_like_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("./")
        || value.starts_with("../")
        || value.starts_with("~/")
        || value.contains('\\')
}

fn reject_unknown_leading_option(value: Option<&String>, command: &str) -> Result<()> {
    if let Some(value) = value {
        if value.starts_with('-') {
            return Err(WorkonError::MissingArgument {
                message: format!("unknown {command} option `{value}`"),
            });
        }
    }

    Ok(())
}

fn reject_unknown_options(parts: &[String], allowed: &[&str], command: &str) -> Result<()> {
    if let Some(value) = parts
        .iter()
        .find(|part| part.starts_with('-') && !allowed.contains(&part.as_str()))
    {
        return Err(WorkonError::MissingArgument {
            message: format!("unknown {command} option `{value}`"),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_args, CliRequest};
    use crate::application::Command;

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
    fn double_dash_allows_goal_starting_with_dash() {
        for (args, input) in [
            (vec!["--", "--machien", "list"], "--machien list"),
            (vec!["--", "--help"], "--help"),
            (vec!["--", "-h"], "-h"),
            (vec!["repos", "--", "--help"], "repos --help"),
        ] {
            let request = parse_args(args.into_iter().map(String::from).collect())
                .expect("args should parse");

            assert_eq!(
                request,
                CliRequest::Command {
                    command: Command::OpenOrCreate {
                        input: input.to_string(),
                        intent_id: None,
                    },
                    machine: false,
                    allow_tui: false,
                }
            );
        }
    }

    #[test]
    fn unknown_options_do_not_create_work() {
        let error = parse_args(vec!["--machien".to_string(), "list".to_string()])
            .expect_err("unknown options should fail");

        assert_eq!(
            error.to_string(),
            "unknown option `--machien`. Use `--` before a work goal that starts with `-`."
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
    fn shell_install_commands_reject_extra_arguments() {
        for (args, expected) in [
            (
                vec!["install-shell".to_string(), "now".to_string()],
                "install-shell does not accept extra arguments. Use `wo \"install-shell ...\"` to create a Work goal that starts with install-shell.",
            ),
            (
                vec!["install-dev-shell".to_string(), "now".to_string()],
                "install-dev-shell does not accept extra arguments. Use `wo \"install-dev-shell ...\"` to create a Work goal that starts with install-dev-shell.",
            ),
        ] {
            let error = parse_args(args).expect_err("shell install extras should fail");

            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn parses_version_requests() {
        for args in [
            vec!["--version".to_string()],
            vec!["-V".to_string()],
            vec!["version".to_string()],
        ] {
            assert_eq!(
                parse_args(args).expect("args should parse"),
                CliRequest::Version
            );
        }
    }

    #[test]
    fn version_command_rejects_extra_arguments() {
        let error = parse_args(vec!["version".to_string(), "extra".to_string()])
            .expect_err("version extras should fail");

        assert_eq!(
            error.to_string(),
            "version does not accept extra arguments. Use `wo \"version ...\"` to create a Work goal that starts with version."
        );
    }

    #[test]
    fn quoted_reserved_command_phrases_are_work_input() {
        for input in [
            "version extra",
            "install-shell now",
            "install-dev-shell now",
        ] {
            assert_eq!(
                parse_args(vec![input.to_string()]).expect("args should parse"),
                CliRequest::Command {
                    command: Command::OpenOrCreate {
                        input: input.to_string(),
                        intent_id: None,
                    },
                    machine: false,
                    allow_tui: false,
                }
            );
        }
    }

    #[test]
    fn intent_option_rejects_command_invocations() {
        for args in [
            vec!["--intent", "investigate", "list"],
            vec!["--intent", "investigate", "archive", "billing"],
            vec!["--intent", "investigate", "repos"],
            vec!["--intent", "investigate", "intent", "list"],
            vec!["--intent", "investigate", "help"],
            vec!["--intent", "investigate", "version"],
            vec!["--intent", "investigate", "install-shell"],
            vec!["--intent", "investigate", "install-dev-shell"],
        ] {
            let args = args.into_iter().map(String::from).collect();
            let error = parse_args(args).expect_err("--intent command misuse should fail");

            assert_eq!(
                error.to_string(),
                "--intent can only be used when creating or opening Work. Use `--` before a Work goal that starts with a command name."
            );
        }
    }

    #[test]
    fn intent_option_without_work_goal_is_actionable() {
        let error = parse_args(vec!["--intent".to_string(), "investigate".to_string()])
            .expect_err("missing intent goal should fail");

        assert_eq!(
            error.to_string(),
            "--intent requires a work goal. Use `wo --intent investigate \"<goal>\"`."
        );
    }

    #[test]
    fn intent_option_allows_command_like_work_goal_after_terminator() {
        assert_eq!(
            parse_args(vec![
                "--intent".to_string(),
                "investigate".to_string(),
                "--".to_string(),
                "list".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::OpenOrCreate {
                    input: "list".to_string(),
                    intent_id: Some("investigate".to_string()),
                },
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

    #[test]
    fn list_command_rejects_extra_arguments() {
        for args in [
            vec!["list".to_string(), "active".to_string()],
            vec!["list".to_string(), "--json".to_string()],
        ] {
            let error = parse_args(args).expect_err("list extras should fail");

            assert_eq!(
                error.to_string(),
                "list does not accept extra arguments. Use `wo \"list ...\"` to create a Work goal that starts with list."
            );
        }
    }

    #[test]
    fn quoted_list_phrase_is_work_input() {
        assert_eq!(
            parse_args(vec!["list active".to_string()]).expect("args should parse"),
            CliRequest::Command {
                command: Command::OpenOrCreate {
                    input: "list active".to_string(),
                    intent_id: None,
                },
                machine: false,
                allow_tui: false,
            }
        );
    }

    #[test]
    fn treats_ctx_as_work_input() {
        for input in ["ctx", "context"] {
            assert_eq!(
                parse_args(vec![input.to_string()]).expect("args should parse"),
                CliRequest::Command {
                    command: Command::OpenOrCreate {
                        input: input.to_string(),
                        intent_id: None,
                    },
                    machine: false,
                    allow_tui: false,
                }
            );
        }
    }

    #[test]
    fn parses_repository_help_requests() {
        for args in [
            vec!["repos".to_string()],
            vec!["repos".to_string(), "--help".to_string()],
            vec!["repos".to_string(), "-h".to_string()],
            vec!["repos".to_string(), "help".to_string()],
            vec!["repos".to_string(), "add".to_string(), "--help".to_string()],
            vec!["help".to_string(), "repos".to_string()],
            vec!["--machine".to_string(), "repos".to_string()],
        ] {
            assert_eq!(
                parse_args(args).expect("args should parse"),
                CliRequest::ReposHelp
            );
        }
    }

    #[test]
    fn parses_general_help_command() {
        assert_eq!(
            parse_args(vec!["help".to_string()]).expect("args should parse"),
            CliRequest::Help
        );
    }

    #[test]
    fn help_command_rejects_extra_arguments() {
        for (args, expected) in [
            (
                vec!["help".to_string(), "repos".to_string(), "extra".to_string()],
                "help repos does not accept extra arguments",
            ),
            (
                vec!["help".to_string(), "intent".to_string(), "extra".to_string()],
                "help intent does not accept extra arguments",
            ),
            (
                vec!["help".to_string(), "unknown".to_string()],
                "unknown help topic `unknown`. Use `wo help`, `wo help repos`, or `wo help intent`.",
            ),
        ] {
            let error = parse_args(args).expect_err("help misuse should fail");

            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn quoted_help_phrase_is_work_input() {
        assert_eq!(
            parse_args(vec!["help repos extra".to_string()]).expect("args should parse"),
            CliRequest::Command {
                command: Command::OpenOrCreate {
                    input: "help repos extra".to_string(),
                    intent_id: None,
                },
                machine: false,
                allow_tui: false,
            }
        );
    }

    #[test]
    fn parses_intent_help_requests() {
        for args in [
            vec!["intent".to_string()],
            vec!["intent".to_string(), "--help".to_string()],
            vec!["intent".to_string(), "-h".to_string()],
            vec!["intent".to_string(), "help".to_string()],
            vec![
                "intent".to_string(),
                "switch".to_string(),
                "--help".to_string(),
            ],
            vec!["help".to_string(), "intent".to_string()],
            vec!["--machine".to_string(), "intent".to_string()],
        ] {
            assert_eq!(
                parse_args(args).expect("args should parse"),
                CliRequest::IntentHelp
            );
        }
    }

    #[test]
    fn parses_intent_commands() {
        assert_eq!(
            parse_args(vec!["intent".to_string(), "list".to_string()]).expect("args should parse"),
            CliRequest::Command {
                command: Command::ListIntents,
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec![
                "intent".to_string(),
                "show".to_string(),
                "investigate".to_string()
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::ShowIntent {
                    intent_id: "investigate".to_string(),
                },
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec![
                "intent".to_string(),
                "switch".to_string(),
                "payment".to_string(),
                "retry".to_string(),
                "review-pr".to_string()
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::SwitchWorkIntent {
                    query: "payment retry".to_string(),
                    intent_id: "review-pr".to_string(),
                },
                machine: false,
                allow_tui: false,
            }
        );

        let error = parse_args(vec!["intent".to_string(), "new".to_string()])
            .expect_err("intent authoring is config-file based");
        assert_eq!(error.to_string(), "intent requires list, show, or switch");
    }

    #[test]
    fn fixed_arity_commands_reject_extra_arguments() {
        let cases = [
            (
                vec!["intent", "show", "investigate", "extra"],
                "intent show accepts exactly one intent id",
            ),
            (
                vec!["intent", "list", "extra"],
                "intent list does not accept extra arguments",
            ),
            (
                vec!["repos", "workspace", "list", "extra"],
                "repos workspace list does not accept extra arguments",
            ),
            (
                vec!["repos", "workspace", "remove", "/tmp/repos", "extra"],
                "repos workspace remove accepts exactly one path",
            ),
            (
                vec!["repos", "help", "extra"],
                "repos help does not accept extra arguments",
            ),
            (
                vec!["intent", "help", "extra"],
                "intent help does not accept extra arguments",
            ),
        ];

        for (args, expected) in cases {
            let args = args.into_iter().map(String::from).collect();
            let error = parse_args(args).expect_err("extra arguments should fail");

            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn repository_commands_reject_unknown_options() {
        for (args, expected) in [
            (
                vec!["repos", "list", "--json"],
                "unknown repos list option `--json`",
            ),
            (
                vec!["repos", "discover", "--json"],
                "unknown repos discover option `--json`",
            ),
            (
                vec!["repos", "link", "--work", "/tmp/repo"],
                "unknown repos link option `--work`",
            ),
            (
                vec!["repos", "add", "--workspcae", "/tmp/repos", "billing", "openai/workon"],
                "unknown repos add option `--workspcae`. Use `--workspace <path>` before the work query.",
            ),
            (
                vec!["repos", "remove", "--froce", "billing", "openai/workon"],
                "unknown repos remove option `--froce`",
            ),
        ] {
            let args = args.into_iter().map(String::from).collect();
            let error = parse_args(args).expect_err("unknown repo option should fail");

            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn parses_force_repository_remove() {
        assert_eq!(
            parse_args(vec![
                "repos".to_string(),
                "remove".to_string(),
                "--force".to_string(),
                "billing".to_string(),
                "openai/workon".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::RemoveWorkRepositories {
                    query: "billing".to_string(),
                    repositories: vec!["openai/workon".to_string()],
                    force: true,
                },
                machine: false,
                allow_tui: false,
            }
        );
    }

    #[test]
    fn repository_add_and_remove_accept_multiword_work_queries() {
        assert_eq!(
            parse_args(vec![
                "repos".to_string(),
                "add".to_string(),
                "payment".to_string(),
                "retry".to_string(),
                "openai/workon".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::AddWorkRepositories {
                    query: "payment retry".to_string(),
                    repositories: vec!["openai/workon".to_string()],
                    workspace: None,
                },
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec![
                "repos".to_string(),
                "remove".to_string(),
                "--force".to_string(),
                "payment".to_string(),
                "retry".to_string(),
                "openai/workon".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::RemoveWorkRepositories {
                    query: "payment retry".to_string(),
                    repositories: vec!["openai/workon".to_string()],
                    force: true,
                },
                machine: false,
                allow_tui: false,
            }
        );
    }

    #[test]
    fn repository_add_and_remove_accept_command_local_terminator() {
        assert_eq!(
            parse_args(vec![
                "repos".to_string(),
                "add".to_string(),
                "payment".to_string(),
                "retry".to_string(),
                "--".to_string(),
                "openai/workon".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::AddWorkRepositories {
                    query: "payment retry".to_string(),
                    repositories: vec!["openai/workon".to_string()],
                    workspace: None,
                },
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec![
                "repos".to_string(),
                "remove".to_string(),
                "--force".to_string(),
                "payment".to_string(),
                "retry".to_string(),
                "--".to_string(),
                "openai/workon".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::RemoveWorkRepositories {
                    query: "payment retry".to_string(),
                    repositories: vec!["openai/workon".to_string()],
                    force: true,
                },
                machine: false,
                allow_tui: false,
            }
        );
    }

    #[test]
    fn repository_link_accepts_multiword_work_query_before_path_like_arguments() {
        assert_eq!(
            parse_args(vec![
                "repos".to_string(),
                "link".to_string(),
                "payment".to_string(),
                "retry".to_string(),
                "/tmp/workon".to_string(),
                "./api".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::LinkWorkRepositories {
                    query: "payment retry".to_string(),
                    paths: vec!["/tmp/workon".into(), "./api".into()],
                },
                machine: false,
                allow_tui: false,
            }
        );
    }

    #[test]
    fn repository_link_accepts_command_local_terminator() {
        assert_eq!(
            parse_args(vec![
                "repos".to_string(),
                "link".to_string(),
                "payment".to_string(),
                "retry".to_string(),
                "--".to_string(),
                "/tmp/workon".to_string(),
                "./api".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::LinkWorkRepositories {
                    query: "payment retry".to_string(),
                    paths: vec!["/tmp/workon".into(), "./api".into()],
                },
                machine: false,
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
