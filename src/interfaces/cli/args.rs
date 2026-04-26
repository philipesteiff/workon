use crate::application::{Command, IntentProfileInput, IntentProfilePatch};
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
        Some("workspace") | Some("workspaces") => {
            parse_repos_workspace_command(input_parts, machine)
        }
        Some("discover") => {
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
        Some("link") => {
            let Some(query) = input_parts.get(2) else {
                return Err(WorkonError::MissingArgument {
                    message: "repos link requires a work query and at least one path".to_string(),
                });
            };
            let paths = input_parts[3..]
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>();
            if paths.is_empty() {
                return Err(WorkonError::MissingArgument {
                    message: "repos link requires at least one path".to_string(),
                });
            }
            Ok(command_request(
                Command::LinkWorkRepositories {
                    query: query.clone(),
                    paths,
                },
                machine,
            ))
        }
        Some("list") => {
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
            let Some(query) = positional.first() else {
                return Err(WorkonError::MissingArgument {
                    message: "repos add requires a work query and at least one repository"
                        .to_string(),
                });
            };
            let repositories = positional[1..].to_vec();
            if repositories.is_empty() {
                return Err(WorkonError::MissingArgument {
                    message: "repos add requires at least one repository".to_string(),
                });
            }
            Ok(command_request(
                Command::AddWorkRepositories {
                    query: query.clone(),
                    repositories,
                    workspace,
                },
                machine,
            ))
        }
        Some("remove") => {
            let force = input_parts[2..].iter().any(|part| part == "--force");
            let parts = input_parts[2..]
                .iter()
                .filter(|part| part.as_str() != "--force")
                .cloned()
                .collect::<Vec<_>>();
            let Some(query) = parts.first() else {
                return Err(WorkonError::MissingArgument {
                    message: "repos remove requires a work query and at least one repository"
                        .to_string(),
                });
            };
            let repositories = parts[1..].to_vec();
            if repositories.is_empty() {
                return Err(WorkonError::MissingArgument {
                    message: "repos remove requires at least one repository".to_string(),
                });
            }
            Ok(command_request(
                Command::RemoveWorkRepositories {
                    query: query.to_string(),
                    repositories,
                    force,
                },
                machine,
            ))
        }
        _ => Err(WorkonError::MissingArgument {
            message: "repos requires list, add, link, discover, workspace, or remove".to_string(),
        }),
    }
}

fn parse_repos_workspace_command(input_parts: &[String], machine: bool) -> Result<CliRequest> {
    match input_parts.get(2).map(String::as_str) {
        Some("list") | None => Ok(command_request(Command::ListRepositoryWorkspaces, machine)),
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
        Some("help") => Ok(CliRequest::IntentHelp),
        Some("list") => Ok(command_request(Command::ListIntents, machine)),
        Some("show") => {
            let Some(intent_id) = input_parts.get(2) else {
                return Err(WorkonError::MissingArgument {
                    message: "intent show requires an intent id".to_string(),
                });
            };
            Ok(command_request(
                Command::ShowIntent {
                    intent_id: intent_id.clone(),
                },
                machine,
            ))
        }
        Some("switch") => {
            let Some(query) = input_parts.get(2) else {
                return Err(WorkonError::MissingArgument {
                    message: "intent switch requires a work query and intent id".to_string(),
                });
            };
            let Some(intent_id) = input_parts.get(3) else {
                return Err(WorkonError::MissingArgument {
                    message: "intent switch requires an intent id".to_string(),
                });
            };
            Ok(command_request(
                Command::SwitchWorkIntent {
                    query: query.clone(),
                    intent_id: intent_id.clone(),
                },
                machine,
            ))
        }
        Some("new") | Some("create") => parse_intent_new(input_parts, machine),
        Some("edit") => parse_intent_edit(input_parts, machine),
        Some("duplicate") | Some("copy") => parse_intent_duplicate(input_parts, machine),
        Some("archive") => {
            let Some(intent_id) = input_parts.get(2) else {
                return Err(WorkonError::MissingArgument {
                    message: "intent archive requires an intent id".to_string(),
                });
            };
            Ok(command_request(
                Command::ArchiveIntent {
                    intent_id: intent_id.clone(),
                },
                machine,
            ))
        }
        _ => Err(WorkonError::MissingArgument {
            message: "intent requires list, show, switch, new, edit, duplicate, or archive"
                .to_string(),
        }),
    }
}

fn parse_intent_new(input_parts: &[String], machine: bool) -> Result<CliRequest> {
    let Some(id) = input_parts.get(2) else {
        return Err(WorkonError::MissingArgument {
            message: "intent new requires an intent id".to_string(),
        });
    };
    let flags = parse_intent_flags(&input_parts[3..], IntentFlagMode::Create)?;
    Ok(command_request(
        Command::CreateIntent {
            input: IntentProfileInput {
                id: id.clone(),
                name: required_flag(flags.name, "--name")?,
                summary: required_flag(flags.summary, "--summary")?,
                skill_weights: flags.skill_weights.unwrap_or_default(),
                mcp_weights: flags.mcp_weights.unwrap_or_default(),
                instructions: flags.instructions.unwrap_or_default(),
            },
        },
        machine,
    ))
}

fn parse_intent_edit(input_parts: &[String], machine: bool) -> Result<CliRequest> {
    let Some(intent_id) = input_parts.get(2) else {
        return Err(WorkonError::MissingArgument {
            message: "intent edit requires an intent id".to_string(),
        });
    };
    let flags = parse_intent_flags(&input_parts[3..], IntentFlagMode::Edit)?;
    Ok(command_request(
        Command::EditIntent {
            intent_id: intent_id.clone(),
            patch: IntentProfilePatch {
                name: flags.name,
                summary: flags.summary,
                skill_weights: flags.skill_weights,
                mcp_weights: flags.mcp_weights,
                instructions: flags.instructions,
            },
        },
        machine,
    ))
}

fn parse_intent_duplicate(input_parts: &[String], machine: bool) -> Result<CliRequest> {
    let Some(source_intent_id) = input_parts.get(2) else {
        return Err(WorkonError::MissingArgument {
            message: "intent duplicate requires a source intent id".to_string(),
        });
    };
    let Some(new_intent_id) = input_parts.get(3) else {
        return Err(WorkonError::MissingArgument {
            message: "intent duplicate requires a new intent id".to_string(),
        });
    };
    let flags = parse_intent_flags(&input_parts[4..], IntentFlagMode::Duplicate)?;
    Ok(command_request(
        Command::DuplicateIntent {
            source_intent_id: source_intent_id.clone(),
            new_intent_id: new_intent_id.clone(),
            name: flags.name,
        },
        machine,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntentFlagMode {
    Create,
    Edit,
    Duplicate,
}

#[derive(Debug, Default)]
struct IntentFlags {
    name: Option<String>,
    summary: Option<String>,
    skill_weights: Option<Vec<String>>,
    mcp_weights: Option<Vec<String>>,
    instructions: Option<Vec<String>>,
}

fn parse_intent_flags(parts: &[String], mode: IntentFlagMode) -> Result<IntentFlags> {
    let mut flags = IntentFlags::default();
    let mut index = 0;
    while index < parts.len() {
        match parts[index].as_str() {
            "--name" => {
                flags.name = Some(flag_value(parts, index, "--name")?);
                index += 2;
            }
            "--summary" if mode != IntentFlagMode::Duplicate => {
                flags.summary = Some(flag_value(parts, index, "--summary")?);
                index += 2;
            }
            "--skill" if mode != IntentFlagMode::Duplicate => {
                push_flag_value(
                    &mut flags.skill_weights,
                    flag_value(parts, index, "--skill")?,
                );
                index += 2;
            }
            "--mcp" if mode != IntentFlagMode::Duplicate => {
                push_flag_value(&mut flags.mcp_weights, flag_value(parts, index, "--mcp")?);
                index += 2;
            }
            "--instruction" if mode != IntentFlagMode::Duplicate => {
                push_flag_value(
                    &mut flags.instructions,
                    flag_value(parts, index, "--instruction")?,
                );
                index += 2;
            }
            "--clear-skills" if mode == IntentFlagMode::Edit => {
                flags.skill_weights = Some(Vec::new());
                index += 1;
            }
            "--clear-mcps" if mode == IntentFlagMode::Edit => {
                flags.mcp_weights = Some(Vec::new());
                index += 1;
            }
            "--clear-instructions" if mode == IntentFlagMode::Edit => {
                flags.instructions = Some(Vec::new());
                index += 1;
            }
            flag => {
                return Err(WorkonError::MissingArgument {
                    message: format!("unknown intent option: {flag}"),
                });
            }
        }
    }
    Ok(flags)
}

fn push_flag_value(values: &mut Option<Vec<String>>, value: String) {
    values.get_or_insert_with(Vec::new).push(value);
}

fn flag_value(parts: &[String], index: usize, flag: &str) -> Result<String> {
    parts
        .get(index + 1)
        .cloned()
        .ok_or_else(|| WorkonError::MissingArgument {
            message: format!("{flag} requires a value"),
        })
}

fn required_flag(value: Option<String>, flag: &str) -> Result<String> {
    value.ok_or_else(|| WorkonError::MissingArgument {
        message: format!("intent new requires {flag}"),
    })
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
        } else {
            positional.push(parts[index].clone());
            index += 1;
        }
    }
    Ok((workspace, positional))
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

    #[test]
    fn parses_repository_help_requests() {
        for args in [
            vec!["repos".to_string()],
            vec!["repos".to_string(), "--help".to_string()],
            vec!["repos".to_string(), "-h".to_string()],
            vec!["repos".to_string(), "help".to_string()],
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
    fn parses_intent_help_requests() {
        for args in [
            vec!["intent".to_string()],
            vec!["intent".to_string(), "--help".to_string()],
            vec!["intent".to_string(), "-h".to_string()],
            vec!["intent".to_string(), "help".to_string()],
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
                "billing".to_string(),
                "review-pr".to_string()
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::SwitchWorkIntent {
                    query: "billing".to_string(),
                    intent_id: "review-pr".to_string(),
                },
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec![
                "intent".to_string(),
                "duplicate".to_string(),
                "investigate".to_string(),
                "debug-prod".to_string(),
                "--name".to_string(),
                "Debug Production".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::DuplicateIntent {
                    source_intent_id: "investigate".to_string(),
                    new_intent_id: "debug-prod".to_string(),
                    name: Some("Debug Production".to_string()),
                },
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec![
                "intent".to_string(),
                "new".to_string(),
                "debug-prod".to_string(),
                "--name".to_string(),
                "Debug Production".to_string(),
                "--summary".to_string(),
                "Diagnose production behavior.".to_string(),
                "--skill".to_string(),
                "systematic-debugging".to_string(),
                "--mcp".to_string(),
                "github".to_string(),
                "--instruction".to_string(),
                "Reproduce before changing code.".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::CreateIntent {
                    input: crate::application::IntentProfileInput {
                        id: "debug-prod".to_string(),
                        name: "Debug Production".to_string(),
                        summary: "Diagnose production behavior.".to_string(),
                        skill_weights: vec!["systematic-debugging".to_string()],
                        mcp_weights: vec!["github".to_string()],
                        instructions: vec!["Reproduce before changing code.".to_string()],
                    },
                },
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec![
                "intent".to_string(),
                "edit".to_string(),
                "debug-prod".to_string(),
                "--summary".to_string(),
                "Use a tighter evidence loop.".to_string(),
                "--clear-mcps".to_string(),
                "--instruction".to_string(),
                "Keep rollback risk visible.".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::EditIntent {
                    intent_id: "debug-prod".to_string(),
                    patch: crate::application::IntentProfilePatch {
                        name: None,
                        summary: Some("Use a tighter evidence loop.".to_string()),
                        skill_weights: None,
                        mcp_weights: Some(Vec::new()),
                        instructions: Some(vec!["Keep rollback risk visible.".to_string()]),
                    },
                },
                machine: false,
                allow_tui: false,
            }
        );

        assert_eq!(
            parse_args(vec![
                "intent".to_string(),
                "archive".to_string(),
                "debug-prod".to_string(),
            ])
            .expect("args should parse"),
            CliRequest::Command {
                command: Command::ArchiveIntent {
                    intent_id: "debug-prod".to_string(),
                },
                machine: false,
                allow_tui: false,
            }
        );
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

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
