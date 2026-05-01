use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn machine_mode_emits_cd_target_for_created_work() {
    let root = temp_root("machine_mode_emits_cd_target_for_created_work");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args([
            "--machine",
            "--intent",
            "investigate",
            "As SE, I want to answer a technical question for my manager that needs investigation across one or more repositories.",
        ])
        .output()
        .expect("wo should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    assert!(stdout.contains("work created: Answer technical question manager"));
    assert!(stdout.contains("__WORKON_CD="));
    assert!(stdout.contains("__WORKON_ROOT="));
    assert!(stdout.contains("__WORKON_TITLE=Answer technical question manager"));
}

#[test]
fn normal_create_without_shell_hook_does_not_start_a_subshell() {
    let root = temp_root("normal_create_without_shell_hook_does_not_start_a_subshell");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .env("WORKON_SHELL_COMMAND", "printf 'INSIDE:%s\\n' \"$PWD\"")
        .args([
            "--intent",
            "investigate",
            "As SE, I want to answer a technical question for my manager that needs investigation across one or more repositories.",
        ])
        .output()
        .expect("wo should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    assert!(stdout.contains("work created: Answer technical question manager"));
    assert!(!stdout.contains("INSIDE:"));
    assert!(!stdout.contains("__WORKON_CD="));
}

#[test]
fn machine_archive_does_not_emit_cd_target() {
    let root = temp_root("machine_archive_does_not_emit_cd_target");

    let create_output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--intent", "investigate", "Archive smoke"])
        .output()
        .expect("wo should run");
    assert!(create_output.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--machine", "archive", "archive-smoke"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    assert!(stdout.contains("work archived: Archive smoke"));
    assert!(stdout.contains("from:"));
    assert!(stdout.contains("to:"));
    assert!(!stdout.contains("__WORKON_CD="));
}

#[test]
fn explicit_list_command_prints_work_summaries() {
    let root = temp_root("explicit_list_command_prints_work_summaries");
    create_work(root.path(), "Investigate billing timeout");
    create_work(root.path(), "Prepare design document");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["list"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    let expected = concat!(
        "  Work                         Intent       Slug                         Path                                      Goal\n",
        "  Investigate billing timeout  investigate  investigate-billing-timeout  .workon/work/investigate-billing-timeout  Investigate billing timeout\n",
        "  Prepare design document      investigate  prepare-design-document      .workon/work/prepare-design-document      Prepare design document\n",
    );

    assert_eq!(stdout, expected);
    assert!(!stdout.contains("__WORKON_CD="));
}

#[test]
fn help_prints_command_summary() {
    let root = temp_root("help_prints_command_summary");

    for args in [vec!["--help"], vec!["help"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_wo"))
            .current_dir(root.path())
            .env("WORKON_ROOT", root.path())
            .args(args)
            .output()
            .expect("wo should run");

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

        assert!(stdout.contains("wo - start, open, and archive work folders"));
        assert!(stdout.contains("wo \"<goal>\"                create work with blank intent"));
        assert!(stdout.contains("wo --intent <id> \"<goal>\"  create work with a selected intent"));
        assert!(stdout.contains("install folder switching for bash, zsh, or fish"));
        assert!(stdout
            .contains("default intents            $WORKON_ROOT/.workon/intents/default/<id>.yaml"));
        assert!(stdout
            .contains("custom intents             $WORKON_ROOT/.workon/intents/custom/<id>.yaml"));
        assert!(!stdout.contains("wo intent new"));
        assert!(!stdout.contains("wo intent edit"));
        assert!(stdout.contains("wo repos add [--workspace <path>] <work> <owner/repo>..."));
        assert!(stdout.contains("create GitHub worktrees from a configured workspace"));
        assert!(!stdout.contains("wo ctx"));
        assert!(stdout.contains("WORKON_ROOT=/path"));
    }
}

#[test]
fn version_prints_package_version() {
    let root = temp_root("version_prints_package_version");

    for args in [vec!["--version"], vec!["version"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_wo"))
            .current_dir(root.path())
            .env("WORKON_ROOT", root.path())
            .args(args)
            .output()
            .expect("wo should run");

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

        assert_eq!(stdout, format!("wo {}\n", env!("CARGO_PKG_VERSION")));
    }
}

#[test]
fn repos_help_prints_usage() {
    let root = temp_root("repos_help_prints_usage");

    for args in [
        vec!["repos"],
        vec!["repos", "--help"],
        vec!["repos", "add", "--help"],
        vec!["help", "repos"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wo"))
            .current_dir(root.path())
            .env("WORKON_ROOT", root.path())
            .args(args)
            .output()
            .expect("wo should run");

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

        assert!(stdout.contains("wo repos - link repositories"));
        assert!(stdout.contains("wo repos list <work-query>"));
        assert!(stdout.contains("wo repos add [--workspace <path>] <work-query> <owner/repo>..."));
        assert!(
            stdout.contains("Create one or more GitHub worktrees in a configured repo workspace")
        );
        assert!(stdout.contains("wo repos workspace add <path>..."));
        assert!(stdout.contains("wo repos remove [--force] <work-query> <owner/repo>..."));
        assert!(stdout.contains("Highlight a Work, press /r"));
        assert!(stdout.contains("tab to"));
        assert!(!stdout.contains("\n  wt\n"));
    }
}

#[test]
fn empty_list_prints_next_step() {
    let root = temp_root("empty_list_prints_next_step");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["list"])
        .output()
        .expect("wo should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    let expected = "\
No active work.
Next: wo \"<goal>\"
Try:  wo --help
";

    assert_eq!(stdout, expected);
}

#[test]
fn create_without_intent_uses_blank_intent() {
    let root = temp_root("create_without_intent_uses_blank_intent");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["Answer billing question"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("work created: Answer billing question"));
    assert!(stdout.contains("intent: blank"));

    let agents_path = root
        .path()
        .join(".workon/work/answer-billing-question/AGENTS.md");
    let agents = fs::read_to_string(agents_path).expect("AGENTS.md should exist");
    assert!(agents.contains("Blank (blank)"));
    assert!(agents.contains("## Instructions\n\n- none"));
}

#[test]
fn unicode_work_goal_creates_readable_slug_and_files() {
    let root = temp_root("unicode_work_goal_creates_readable_slug_and_files");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--intent", "investigate", "Réparer déploiement côté client"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("work created: Réparer déploiement côté client"));
    assert!(stdout.contains("slug: réparer-déploiement-côté-client"));

    let work_path = root
        .path()
        .join(".workon/work/réparer-déploiement-côté-client");
    assert!(work_path.join("AGENTS.md").is_file());
    let agents = fs::read_to_string(work_path.join("AGENTS.md"))
        .expect("unicode work AGENTS.md should be readable");
    assert!(agents.contains("Réparer déploiement côté client"));
}

#[test]
fn custom_intents_load_from_config_file() {
    let root = temp_root("custom_intents_load_from_config_file");
    write_custom_intents_config(root.path());

    let list = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["intent", "list"])
        .output()
        .expect("wo should run");

    assert!(list.status.success());
    let stdout = String::from_utf8(list.stdout).expect("stdout should be utf8");
    assert!(stdout.contains(
        "debug-production  custom  Debug Production  Diagnose production behavior from evidence."
    ));

    let create = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--intent", "debug-production", "Debug production checkout"])
        .output()
        .expect("wo should run");

    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let stdout = String::from_utf8(create.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("intent: debug-production"));

    let agents = fs::read_to_string(
        root.path()
            .join(".workon/work/debug-production-checkout/AGENTS.md"),
    )
    .expect("AGENTS.md should exist");
    assert!(agents.contains("Debug Production (debug-production)"));
    assert!(agents.contains("Keep rollback risk visible."));
}

#[test]
fn cli_errors_are_actionable() {
    let root = temp_root("cli_errors_are_actionable");

    let unknown_intent = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--intent", "missing", "Answer billing question"])
        .output()
        .expect("wo should run");

    assert!(!unknown_intent.status.success());
    let stderr = String::from_utf8(unknown_intent.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("unknown intent `missing`"));
    assert!(stderr.contains("Available:"));
    assert!(stderr.contains("blank"));
    assert!(stderr.contains("investigate"));

    let not_found = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["archive", "missing-work"])
        .output()
        .expect("wo should run");

    assert!(!not_found.status.success());
    let stderr = String::from_utf8(not_found.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("work not found: `missing-work`"));
    assert!(stderr.contains("Run `wo list`"));
}

#[test]
fn fixed_arity_cli_errors_reject_extra_arguments() {
    let root = temp_root("fixed_arity_cli_errors_reject_extra_arguments");

    for (args, expected) in [
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
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wo"))
            .current_dir(root.path())
            .env("WORKON_ROOT", root.path())
            .args(args)
            .output()
            .expect("wo should run");

        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(stderr.contains(expected), "stderr:\n{stderr}");
    }
}

#[test]
fn intent_switch_accepts_multiword_work_query() {
    let root = temp_root("intent_switch_accepts_multiword_work_query");
    create_work(root.path(), "Payment retry latency");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["intent", "switch", "payment", "retry", "brainstorm"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("work intent switched: Payment retry latency"));
    assert!(stdout.contains("to: brainstorm"));
}

#[test]
fn repository_cli_errors_reject_unknown_options() {
    let root = temp_root("repository_cli_errors_reject_unknown_options");

    for (args, expected) in [
        (
            vec!["repos", "list", "--json"],
            "unknown repos list option `--json`",
        ),
        (
            vec![
                "repos",
                "add",
                "--workspcae",
                "/tmp/repos",
                "billing",
                "openai/workon",
            ],
            "unknown repos add option `--workspcae`",
        ),
        (
            vec!["repos", "remove", "--froce", "billing", "openai/workon"],
            "unknown repos remove option `--froce`",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wo"))
            .current_dir(root.path())
            .env("WORKON_ROOT", root.path())
            .args(args)
            .output()
            .expect("wo should run");

        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(stderr.contains(expected), "stderr:\n{stderr}");
    }
}

#[test]
fn unknown_option_does_not_create_work() {
    let root = temp_root("unknown_option_does_not_create_work");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--machien", "list"])
        .output()
        .expect("wo should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("unknown option `--machien`"),
        "stderr:\n{stderr}"
    );
    assert!(
        !root.path().join(".workon/work/machien-list").exists(),
        "unknown option should not create a work folder"
    );
}

#[test]
fn double_dash_allows_help_like_work_goal() {
    let root = temp_root("double_dash_allows_help_like_work_goal");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--", "--help"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("work created: --help"));
    assert!(
        !stdout.contains("wo - start, open, and archive work folders"),
        "option terminator should route --help as Work input"
    );
    assert!(root.path().join(".workon/work/help").is_dir());
}

#[test]
fn list_command_extra_arguments_do_not_create_work() {
    let root = temp_root("list_command_extra_arguments_do_not_create_work");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["list", "active"])
        .output()
        .expect("wo should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("list does not accept extra arguments"),
        "stderr:\n{stderr}"
    );
    assert!(
        !root.path().join(".workon/work/list-active").exists(),
        "list command misuse should not create a work folder"
    );
}

#[test]
fn reserved_command_extra_arguments_do_not_create_work() {
    let root = temp_root("reserved_command_extra_arguments_do_not_create_work");

    for (args, expected, slug) in [
        (
            vec!["version", "extra"],
            "version does not accept extra arguments",
            "version-extra",
        ),
        (
            vec!["install-shell", "now"],
            "install-shell does not accept extra arguments",
            "install-shell-now",
        ),
        (
            vec!["install-dev-shell", "now"],
            "install-dev-shell does not accept extra arguments",
            "install-dev-shell-now",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wo"))
            .current_dir(root.path())
            .env("WORKON_ROOT", root.path())
            .args(args)
            .output()
            .expect("wo should run");

        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(stderr.contains(expected), "stderr:\n{stderr}");
        assert!(
            !root.path().join(".workon/work").join(slug).exists(),
            "reserved command misuse should not create a work folder"
        );
    }
}

#[test]
fn help_command_extra_arguments_do_not_create_work() {
    let root = temp_root("help_command_extra_arguments_do_not_create_work");

    for (args, expected, slug) in [
        (
            vec!["help", "repos", "extra"],
            "help repos does not accept extra arguments",
            "help-repos-extra",
        ),
        (
            vec!["help", "intent", "extra"],
            "help intent does not accept extra arguments",
            "help-intent-extra",
        ),
        (
            vec!["help", "unknown"],
            "unknown help topic `unknown`",
            "help-unknown",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wo"))
            .current_dir(root.path())
            .env("WORKON_ROOT", root.path())
            .args(args)
            .output()
            .expect("wo should run");

        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(stderr.contains(expected), "stderr:\n{stderr}");
        assert!(
            !root.path().join(".workon/work").join(slug).exists(),
            "help command misuse should not create a work folder"
        );
    }
}

#[test]
fn intent_option_with_commands_is_rejected_without_creating_work() {
    let root = temp_root("intent_option_with_commands_is_rejected_without_creating_work");

    for (args, slug) in [
        (vec!["--intent", "investigate", "list"], "list"),
        (
            vec!["--intent", "investigate", "archive", "billing"],
            "archive-billing",
        ),
        (vec!["--intent", "investigate", "repos"], "repos"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wo"))
            .current_dir(root.path())
            .env("WORKON_ROOT", root.path())
            .args(args)
            .output()
            .expect("wo should run");

        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains("--intent can only be used when creating or opening Work"),
            "stderr:\n{stderr}"
        );
        assert!(
            !root.path().join(".workon/work").join(slug).exists(),
            "--intent command misuse should not create a work folder"
        );
    }
}

#[test]
fn intent_option_without_goal_is_actionable() {
    let root = temp_root("intent_option_without_goal_is_actionable");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--intent", "investigate"])
        .output()
        .expect("wo should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("--intent requires a work goal"),
        "stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("wo --intent investigate \"<goal>\""),
        "stderr:\n{stderr}"
    );
}

#[test]
fn ambiguous_work_error_lists_slug_commands() {
    let root = temp_root("ambiguous_work_error_lists_slug_commands");
    create_work(root.path(), "Answer billing question");
    create_work(root.path(), "Investigate billing issue");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["archive", "billing"])
        .output()
        .expect("wo should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");

    assert!(stderr.contains("work query `billing` matched multiple works"));
    assert!(stderr.contains("Use a slug:"));
    assert!(stderr.contains("wo answer-billing-question  # Answer billing question"));
    assert!(stderr.contains("wo investigate-billing-issue  # Investigate billing issue"));
}

#[test]
fn repos_add_list_and_remove_use_real_wo_binary_with_fake_tools() {
    let root = temp_root("repos_add_list_and_remove_use_real_wo_binary");
    let fake_bin = fake_repo_tools();
    let fake_path = path_with_prepended(fake_bin.path());
    let log_path = root.path().join("tool.log");
    let workspace_path = root.path().join("repo-workspace");
    let second_workspace_path = root.path().join("repo-workspace-client");
    fs::write(&log_path, "").expect("tool log should initialize");
    create_work(root.path(), "Repository context cli smoke");

    let workspace = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args([
            "repos",
            "workspace",
            "add",
            workspace_path.to_str().expect("workspace path utf8"),
            second_workspace_path
                .to_str()
                .expect("second workspace path utf8"),
        ])
        .output()
        .expect("wo repos workspace add should run");
    assert!(
        workspace.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&workspace.stdout),
        String::from_utf8_lossy(&workspace.stderr)
    );

    let add = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .env("PATH", &fake_path)
        .env("WORKON_FAKE_LOG", &log_path)
        .args([
            "repos",
            "add",
            "--workspace",
            workspace_path.to_str().expect("workspace path utf8"),
            "repository",
            "context",
            "cli",
            "smoke",
            "--",
            "openai/workon",
        ])
        .output()
        .expect("wo repos add should run");

    assert!(
        add.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );
    let add_stdout = String::from_utf8(add.stdout).expect("stdout should be utf8");
    assert!(add_stdout.contains("repositories added: Repository context cli smoke"));
    assert!(add_stdout.contains("openai/workon"));

    let list = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .env("PATH", &fake_path)
        .env("WORKON_FAKE_LOG", &log_path)
        .args(["repos", "list", "repository", "context", "cli", "smoke"])
        .output()
        .expect("wo repos list should run");

    assert!(
        list.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&list.stdout),
        String::from_utf8_lossy(&list.stderr)
    );
    let list_stdout = String::from_utf8(list.stdout).expect("stdout should be utf8");
    assert!(list_stdout.contains("repositories for work: Repository context cli smoke"));
    assert!(list_stdout.contains("openai/workon"));
    assert!(list_stdout.contains("workon/repository-context-cli-smoke"));

    let remove = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .env("PATH", &fake_path)
        .env("WORKON_FAKE_LOG", &log_path)
        .args([
            "repos",
            "remove",
            "--force",
            "repository",
            "context",
            "cli",
            "smoke",
            "--",
            "openai/workon",
        ])
        .output()
        .expect("wo repos remove should run");

    assert!(
        remove.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&remove.stdout),
        String::from_utf8_lossy(&remove.stderr)
    );
    let remove_stdout = String::from_utf8(remove.stdout).expect("stdout should be utf8");
    assert!(remove_stdout.contains("repositories removed: Repository context cli smoke"));
    assert!(remove_stdout.contains("openai/workon"));

    let metadata = fs::read_to_string(
        root.path()
            .join(".workon/work/repository-context-cli-smoke/workon.repos.json"),
    )
    .expect("repo metadata should exist");
    assert!(!metadata.contains("openai/workon"));

    let log = fs::read_to_string(log_path).expect("tool log should exist");
    assert!(log.contains("gh repo view openai/workon"));
    assert!(log.contains("gh repo clone openai/workon"));
    assert!(log.contains("git -C"));
    assert!(!log.contains("worktree remove --force"));
}

#[test]
fn repos_link_uses_real_wo_binary_with_existing_local_checkout() {
    let root = temp_root("repos_link_uses_real_wo_binary_with_existing_local_checkout");
    let fake_bin = fake_repo_tools();
    let fake_path = path_with_prepended(fake_bin.path());
    let log_path = root.path().join("tool.log");
    let checkout_path = root.path().join("existing checkout");
    fs::write(&log_path, "").expect("tool log should initialize");
    fs::create_dir_all(&checkout_path).expect("checkout path should exist");
    fs::write(
        checkout_path.join(".workon-current-branch"),
        "feature/local-link\n",
    )
    .expect("fake branch marker should be written");
    fs::write(
        checkout_path.join(".workon-origin-url"),
        "https://github.com/openai/workon.git\n",
    )
    .expect("fake origin marker should be written");
    create_work(root.path(), "Local repository link smoke");

    let link = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .env("PATH", &fake_path)
        .env("WORKON_FAKE_LOG", &log_path)
        .args([
            "repos",
            "link",
            "local",
            "repository",
            "link",
            "--",
            checkout_path.to_str().expect("checkout path utf8"),
        ])
        .output()
        .expect("wo repos link should run");

    assert!(
        link.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&link.stdout),
        String::from_utf8_lossy(&link.stderr)
    );
    let link_stdout = String::from_utf8(link.stdout).expect("stdout should be utf8");
    assert!(link_stdout.contains("repositories added: Local repository link smoke"));
    assert!(link_stdout.contains("openai/workon"));

    let list = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .env("PATH", &fake_path)
        .env("WORKON_FAKE_LOG", &log_path)
        .args(["repos", "list", "local", "repository", "link"])
        .output()
        .expect("wo repos list should run");

    assert!(
        list.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&list.stdout),
        String::from_utf8_lossy(&list.stderr)
    );
    let list_stdout = String::from_utf8(list.stdout).expect("stdout should be utf8");
    assert!(list_stdout.contains("repositories for work: Local repository link smoke"));
    assert!(list_stdout.contains("openai/workon"));
    assert!(list_stdout.contains("feature/local-link"));

    let log = fs::read_to_string(log_path).expect("tool log should exist");
    assert!(log.contains("git -C"));
    assert!(log.contains("remote get-url origin"));
}

#[test]
fn repos_add_reports_missing_gh_cli_as_actionable_error() {
    let root = temp_root("repos_add_reports_missing_gh_cli_as_actionable_error");
    let empty_path = temp_root("missing_gh_path");
    let workspace_path = root.path().join("repo-workspace");
    create_work(root.path(), "Missing GitHub cli smoke");

    let workspace = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args([
            "repos",
            "workspace",
            "add",
            workspace_path.to_str().expect("workspace path utf8"),
        ])
        .output()
        .expect("wo repos workspace add should run");
    assert!(
        workspace.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&workspace.stdout),
        String::from_utf8_lossy(&workspace.stderr)
    );

    let add = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .env("PATH", empty_path.path())
        .args([
            "repos",
            "add",
            "--workspace",
            workspace_path.to_str().expect("workspace path utf8"),
            "missing-github-cli-smoke",
            "openai/workon",
        ])
        .output()
        .expect("wo repos add should run");

    assert!(!add.status.success());
    let stderr = String::from_utf8(add.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("required command `gh` was not found in PATH"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn repos_add_preserves_gh_auth_failure_guidance() {
    let root = temp_root("repos_add_preserves_gh_auth_failure_guidance");
    let fake_bin = fake_gh_auth_failure_tools();
    let workspace_path = root.path().join("repo-workspace");
    create_work(root.path(), "GitHub auth failure smoke");

    let workspace = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args([
            "repos",
            "workspace",
            "add",
            workspace_path.to_str().expect("workspace path utf8"),
        ])
        .output()
        .expect("wo repos workspace add should run");
    assert!(
        workspace.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&workspace.stdout),
        String::from_utf8_lossy(&workspace.stderr)
    );

    let add = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .env("PATH", path_with_prepended(fake_bin.path()))
        .args([
            "repos",
            "add",
            "--workspace",
            workspace_path.to_str().expect("workspace path utf8"),
            "github-auth-failure-smoke",
            "openai/workon",
        ])
        .output()
        .expect("wo repos add should run");

    assert!(!add.status.success());
    let stderr = String::from_utf8(add.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("gh auth login"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("repository context command failed: gh repo view openai/workon"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn default_root_uses_home_not_current_directory() {
    let home = temp_root("default_root_uses_home_not_current_directory_home");
    let cwd = temp_root("default_root_uses_home_not_current_directory_cwd");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env_remove("WORKON_ROOT")
        .args(["--intent", "investigate", "Home root smoke"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(home.path().join(".workon/work/home-root-smoke").is_dir());
    assert!(!cwd.path().join(".workon/work/home-root-smoke").exists());
}

#[test]
fn workon_root_env_overrides_home_root() {
    let home = temp_root("workon_root_env_overrides_home_root_home");
    let override_root = temp_root("workon_root_env_overrides_home_root_override");
    let cwd = temp_root("workon_root_env_overrides_home_root_cwd");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env("WORKON_ROOT", override_root.path())
        .args(["--intent", "investigate", "Override root smoke"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(override_root
        .path()
        .join(".workon/work/override-root-smoke")
        .is_dir());
    assert!(!home
        .path()
        .join(".workon/work/override-root-smoke")
        .exists());
    assert!(!cwd.path().join(".workon/work/override-root-smoke").exists());
}

#[test]
fn install_shell_writes_sourceable_prod_integration() {
    let home = temp_root("install_shell_writes_sourceable_prod_integration_home");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .env("HOME", home.path())
        .env("SHELL", "/bin/bash")
        .args(["install-shell"])
        .output()
        .expect("wo should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    let script = home.path().join(".workon/shell/wo");
    let startup = home.path().join(".bashrc");
    let script_content = fs::read_to_string(&script).expect("script should be readable");
    let startup_content = fs::read_to_string(&startup).expect("startup file should be readable");

    assert!(script_content.contains("wo()"));
    assert!(script_content.contains("--machine"));
    assert!(startup_content.contains("[ -f "));
    assert!(startup_content.contains(".workon/shell/wo"));
    assert!(stdout.contains("startup file updated:"));
}

#[test]
fn install_shell_prints_source_command_with_shell_quoted_path() {
    let home = temp_root("install shell prints quoted source's home");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .env("HOME", home.path())
        .env("SHELL", "zsh")
        .args(["install-shell"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let script = home.path().join(".workon/shell/wo");

    assert!(
        stdout.contains(&format!("  source {}", shell_quote(&script))),
        "stdout:\n{stdout}"
    );
}

#[test]
fn installed_prod_shell_function_switches_to_home_root() {
    let home = temp_root("installed_prod_shell_function_switches_to_home_root_home");
    let cwd = temp_root("installed_prod_shell_function_switches_to_home_root_cwd");
    let script = install_prod_shell(home.path());
    let expected_work = home.path().join(".workon/work/prod-hook-home-smoke");

    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "source {}; cd {}; wo --intent investigate 'Prod hook home smoke'; printf 'PWD:%s\\n' \"$PWD\"; test \"$PWD\" = {}",
            shell_quote(&script),
            shell_quote(cwd.path()),
            shell_quote(&expected_work),
        ))
        .env("HOME", home.path())
        .env_remove("WORKON_ROOT")
        .output()
        .expect("shell should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(expected_work.is_dir());
    assert!(!cwd
        .path()
        .join(".workon/work/prod-hook-home-smoke")
        .exists());
}

#[test]
fn install_shell_writes_sourceable_zsh_integration() {
    if !shell_available("zsh") {
        eprintln!("skipping zsh shell integration test: zsh is not installed");
        return;
    }

    let home = temp_root("install_shell_writes_sourceable_zsh_integration_home");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .env("HOME", home.path())
        .env("SHELL", "zsh")
        .args(["install-shell"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let script = home.path().join(".workon/shell/wo");
    let startup = home.path().join(".zshrc");
    let script_content = fs::read_to_string(&script).expect("script should be readable");
    let startup_content = fs::read_to_string(&startup).expect("startup file should be readable");

    assert!(script_content.contains("wo()"));
    assert!(script_content.contains("--machine"));
    assert!(startup_content.contains("[ -f "));
    assert!(startup_content.contains(".workon/shell/wo"));
}

#[test]
fn install_shell_writes_sourceable_fish_integration() {
    let home = temp_root("install_shell_writes_sourceable_fish_integration_home");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .env("HOME", home.path())
        .env("SHELL", "/usr/local/bin/fish")
        .args(["install-shell"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let script = home.path().join(".workon/shell/wo");
    let startup = home.path().join(".config/fish/config.fish");
    let script_content = fs::read_to_string(&script).expect("script should be readable");
    let startup_content = fs::read_to_string(&startup).expect("startup file should be readable");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    assert!(script_content.contains("function wo"));
    assert!(script_content.contains("--machine"));
    assert!(startup_content.contains("test -f "));
    assert!(startup_content.contains("; and source "));
    assert!(startup_content.contains(".workon/shell/wo"));
    assert!(stdout.contains("startup file updated:"));
    assert!(stdout.contains(".config/fish/config.fish"));
}

#[test]
fn install_shell_without_shell_reports_supported_shells() {
    let home = temp_root("install_shell_without_shell_reports_supported_shells_home");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .env("HOME", home.path())
        .env_remove("SHELL")
        .args(["install-shell"])
        .output()
        .expect("wo should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");

    assert!(stderr.contains("SHELL is required to install shell integration"));
    assert!(stderr.contains("Supported shells: bash, zsh, fish."));
}

#[test]
fn installed_prod_zsh_shell_function_switches_to_home_root() {
    if !shell_available("zsh") {
        eprintln!("skipping zsh shell integration test: zsh is not installed");
        return;
    }

    let home = temp_root("installed_prod_zsh_shell_function_switches_to_home_root_home");
    let cwd = temp_root("installed_prod_zsh_shell_function_switches_to_home_root_cwd");
    let script = install_prod_shell_for(home.path(), "zsh");
    let expected_work = home.path().join(".workon/work/prod-zsh-hook-smoke");

    let output = Command::new("zsh")
        .arg("-f")
        .arg("-c")
        .arg(format!(
            "source {}; cd {}; wo --intent investigate 'Prod zsh hook smoke'; printf 'PWD:%s\\n' \"$PWD\"; test \"$PWD\" = {}",
            shell_quote(&script),
            shell_quote(cwd.path()),
            shell_quote(&expected_work),
        ))
        .env("HOME", home.path())
        .env_remove("WORKON_ROOT")
        .output()
        .expect("zsh should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(expected_work.is_dir());
    assert!(!cwd.path().join(".workon/work/prod-zsh-hook-smoke").exists());
}

#[test]
fn installed_dev_shell_function_switches_current_shell() {
    let home = temp_root("installed_dev_shell_function_switches_current_shell_home");
    let root = temp_root("installed_dev_shell_function_switches_current_shell_root");
    let script = install_dev_shell(home.path());
    let expected_work = home.path().join(".workon/work/hook-navigation-smoke");

    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "source {}; cd {}; wo --intent investigate 'Hook navigation smoke'; printf 'PWD:%s\\n' \"$PWD\"; test \"$PWD\" = {}",
            shell_quote(&script),
            shell_quote(root.path()),
            shell_quote(&expected_work),
        ))
        .env("HOME", home.path())
        .env_remove("WORKON_ROOT")
        .apply_cargo_env()
        .output()
        .expect("shell should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("work created: Hook navigation smoke"));
    assert!(stdout.contains("PWD:"));
    assert!(stdout.contains("hook-navigation-smoke"));
    assert!(expected_work.is_dir());
    assert!(!root
        .path()
        .join(".workon/work/hook-navigation-smoke")
        .exists());
}

#[test]
fn installed_dev_shell_intercepts_just_wo() {
    let home = temp_root("installed_dev_shell_intercepts_just_wo_home");
    let root = temp_root("installed_dev_shell_intercepts_just_wo_root");
    let script = install_dev_shell(home.path());
    let expected_work = home.path().join(".workon/work/just-hook-navigation-smoke");

    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "source {}; export WORKON_DEV_ROOT={}; cd {}; just wo --intent investigate 'Just hook navigation smoke'; printf 'PWD:%s\\n' \"$PWD\"; test \"$PWD\" = {}",
            shell_quote(&script),
            shell_quote(root.path()),
            shell_quote(root.path()),
            shell_quote(&expected_work),
        ))
        .env("HOME", home.path())
        .env_remove("WORKON_ROOT")
        .apply_cargo_env()
        .output()
        .expect("shell should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("work created: Just hook navigation smoke"));
    assert!(stdout.contains("PWD:"));
    assert!(stdout.contains("just-hook-navigation-smoke"));
    assert!(expected_work.is_dir());
    assert!(!root
        .path()
        .join(".workon/work/just-hook-navigation-smoke")
        .exists());
}

#[test]
fn installed_dev_zsh_shell_intercepts_just_wo() {
    if !shell_available("zsh") {
        eprintln!("skipping zsh shell integration test: zsh is not installed");
        return;
    }

    let home = temp_root("installed_dev_zsh_shell_intercepts_just_wo_home");
    let root = temp_root("installed_dev_zsh_shell_intercepts_just_wo_root");
    let script = install_dev_shell_for(home.path(), "zsh");
    let expected_work = home.path().join(".workon/work/just-zsh-hook-smoke");

    let output = Command::new("zsh")
        .arg("-f")
        .arg("-c")
        .arg(format!(
            "source {}; export WORKON_DEV_ROOT={}; cd {}; just wo --intent investigate 'Just zsh hook smoke'; printf 'PWD:%s\\n' \"$PWD\"; test \"$PWD\" = {}",
            shell_quote(&script),
            shell_quote(root.path()),
            shell_quote(root.path()),
            shell_quote(&expected_work),
        ))
        .env("HOME", home.path())
        .env_remove("WORKON_ROOT")
        .apply_cargo_env()
        .output()
        .expect("zsh should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("work created: Just zsh hook smoke"));
    assert!(stdout.contains("PWD:"));
    assert!(stdout.contains("just-zsh-hook-smoke"));
    assert!(expected_work.is_dir());
    assert!(!root
        .path()
        .join(".workon/work/just-zsh-hook-smoke")
        .exists());
}

fn install_prod_shell(home: &Path) -> PathBuf {
    install_prod_shell_for(home, "/bin/bash")
}

fn install_prod_shell_for(home: &Path, shell: &str) -> PathBuf {
    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .env("HOME", home)
        .env("SHELL", shell)
        .env_remove("WORKON_ROOT")
        .args(["install-shell"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    home.join(".workon/shell/wo")
}

fn install_dev_shell(home: &Path) -> PathBuf {
    install_dev_shell_for(home, "/bin/bash")
}

fn install_dev_shell_for(home: &Path, shell: &str) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .env("HOME", home)
        .env("SHELL", shell)
        .env("WORKON_DEV_MANIFEST", manifest)
        .args(["install-dev-shell"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    home.join(".workon/shell/wo-dev")
}

fn shell_available(shell: &str) -> bool {
    Command::new(shell)
        .arg("-c")
        .arg("exit 0")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn create_work(root: &Path, goal: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root)
        .env("WORKON_ROOT", root)
        .args(["--intent", "investigate", goal])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_custom_intents_config(root: &Path) {
    let path = root.join(".workon/intents/custom/debug-production.yaml");
    fs::create_dir_all(path.parent().expect("config parent should exist"))
        .expect("config parent should be created");
    fs::write(
        path,
        r#"id: debug-production
name: Debug Production
summary: Diagnose production behavior from evidence.
skill_weights:
  - systematic-debugging
mcp_weights:
  - github
  - logs
instructions:
  - Reproduce before changing code.
  - Keep rollback risk visible.
"#,
    )
    .expect("custom intents config should write");
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn temp_root(name: &str) -> TempRoot {
    static TEMP_ROOT_COUNTER: AtomicUsize = AtomicUsize::new(0);

    let counter = TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "workon-cli-test-{name}-{}-{counter}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("temp root should be created");
    TempRoot { path }
}

fn fake_repo_tools() -> TempRoot {
    let fake_bin = temp_root("fake_repo_tools");
    fs::write(fake_bin.path().join("gh"), fake_gh_script()).expect("gh fake should be written");
    fs::write(fake_bin.path().join("git"), fake_git_script()).expect("git fake should be written");

    for name in ["gh", "git"] {
        let path = fake_bin.path().join(name);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path)
                .expect("fake tool metadata should read")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("fake tool should be executable");
        }
    }

    fake_bin
}

fn fake_gh_auth_failure_tools() -> TempRoot {
    let fake_bin = temp_root("fake_gh_auth_failure_tools");
    fs::write(fake_bin.path().join("gh"), fake_gh_auth_failure_script())
        .expect("gh auth failure fake should be written");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = fake_bin.path().join("gh");
        let mut permissions = fs::metadata(&path)
            .expect("fake gh metadata should read")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("fake gh should be executable");
    }

    fake_bin
}

fn path_with_prepended(path: &Path) -> std::ffi::OsString {
    let mut paths = vec![path.to_path_buf()];
    if let Some(previous) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&previous));
    }
    std::env::join_paths(paths).expect("PATH should join")
}

fn fake_gh_script() -> &'static str {
    r#"#!/bin/sh
printf 'gh %s\n' "$*" >> "$WORKON_FAKE_LOG"
if [ "$1" = "repo" ] && [ "$2" = "view" ]; then
  repo="$3"
  printf '{"nameWithOwner":"%s","defaultBranchRef":{"name":"main"},"url":"https://github.com/%s","sshUrl":"git@github.com:%s.git"}\n' "$repo" "$repo" "$repo"
  exit 0
fi
if [ "$1" = "repo" ] && [ "$2" = "clone" ]; then
  mkdir -p "$4"
  exit 0
fi
exit 2
"#
}

fn fake_gh_auth_failure_script() -> &'static str {
    r#"#!/bin/sh
printf 'gh %s\n' "$*" >&2
printf 'The token in default is invalid.\n' >&2
printf 'To re-authenticate, run: gh auth login -h github.com\n' >&2
exit 4
"#
}

fn fake_git_script() -> &'static str {
    r#"#!/bin/sh
printf 'git %s\n' "$*" >> "$WORKON_FAKE_LOG"
if [ "$1" = "-C" ]; then
  cwd="$2"
  shift 2
fi
if [ "$1" = "fetch" ]; then
  exit 0
fi
if [ "$1" = "branch" ] && [ "$2" = "--list" ]; then
  exit 0
fi
if [ "$1" = "branch" ]; then
  if [ "$2" = "--show-current" ]; then
    if [ -n "$cwd" ] && [ -f "$cwd/.workon-current-branch" ]; then
      cat "$cwd/.workon-current-branch"
    fi
    exit 0
  fi
  exit 0
fi
if [ "$1" = "remote" ] && [ "$2" = "get-url" ] && [ "$3" = "origin" ]; then
  if [ -n "$cwd" ] && [ -f "$cwd/.workon-origin-url" ]; then
    cat "$cwd/.workon-origin-url"
  fi
  exit 0
fi
if [ "$1" = "worktree" ] && [ "$2" = "add" ]; then
  mkdir -p "$3"
  printf '%s\n' "$4" > "$3/.workon-current-branch"
  printf 'https://github.com/openai/workon.git\n' > "$3/.workon-origin-url"
  exit 0
fi
exit 0
"#
}

fn shell_quote(path: &Path) -> String {
    let value = path.display().to_string();
    format!("'{}'", value.replace('\'', "'\\''"))
}

trait CargoEnvCommand {
    fn apply_cargo_env(&mut self) -> &mut Self;
}

impl CargoEnvCommand for Command {
    fn apply_cargo_env(&mut self) -> &mut Self {
        if let Some(cargo_home) = preserved_home_child("CARGO_HOME", ".cargo") {
            self.env("CARGO_HOME", cargo_home);
        }
        if let Some(rustup_home) = preserved_home_child("RUSTUP_HOME", ".rustup") {
            self.env("RUSTUP_HOME", rustup_home);
        }
        self
    }
}

fn preserved_home_child(env_name: &str, child: &str) -> Option<PathBuf> {
    std::env::var_os(env_name)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(child)))
}
