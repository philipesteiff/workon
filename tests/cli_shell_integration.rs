use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--help"])
        .output()
        .expect("wo should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    assert!(stdout.contains("wo - start, open, and archive work folders"));
    assert!(stdout.contains("wo --intent <id> \"<goal>\"  create work"));
    assert!(stdout.contains("wo repos add <work> <owner/repo>..."));
    assert!(stdout.contains("attach GitHub repos as worktrees"));
    assert!(stdout.contains("WORKON_ROOT=/path"));
}

#[test]
fn repos_help_prints_usage() {
    let root = temp_root("repos_help_prints_usage");

    for args in [
        vec!["repos"],
        vec!["repos", "--help"],
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

        assert!(stdout.contains("wo repos - attach GitHub repositories"));
        assert!(stdout.contains("wo repos list <work-query>"));
        assert!(stdout.contains("wo repos add <work-query> <owner/repo>..."));
        assert!(stdout.contains("wo repos remove <work-query> <owner/repo>..."));
        assert!(stdout.contains("Highlight a Work, press /r"));
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
Next: wo --intent <intent-id> \"<goal>\"
Try:  wo --help
";

    assert_eq!(stdout, expected);
}

#[test]
fn cli_errors_are_actionable() {
    let root = temp_root("cli_errors_are_actionable");

    let missing_intent = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["Answer billing question"])
        .output()
        .expect("wo should run");

    assert!(!missing_intent.status.success());
    let stderr = String::from_utf8(missing_intent.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("intent required for new work"));
    assert!(stderr.contains("Use: wo --intent <intent-id> \"<goal>\""));
    assert!(stderr.contains("Available: investigate"));

    let unknown_intent = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .args(["--intent", "missing", "Answer billing question"])
        .output()
        .expect("wo should run");

    assert!(!unknown_intent.status.success());
    let stderr = String::from_utf8(unknown_intent.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("unknown intent `missing`"));
    assert!(stderr.contains("Available: investigate"));

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
    fs::write(&log_path, "").expect("tool log should initialize");
    create_work(root.path(), "Repository context cli smoke");

    let add = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_ROOT", root.path())
        .env("PATH", &fake_path)
        .env("WORKON_FAKE_LOG", &log_path)
        .args([
            "repos",
            "add",
            "repository-context-cli-smoke",
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
        .args(["repos", "list", "repository-context-cli-smoke"])
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
            "repository-context-cli-smoke",
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
    assert!(log.contains("wt -C"));
    assert!(log.contains("remove --no-delete-branch"));
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
        .args(["install-shell"])
        .output()
        .expect("wo should run");

    assert!(output.status.success());

    let script = home.path().join(".workon/shell/zsh/wo.zsh");
    let zshrc = home.path().join(".zshrc");
    let script_content = fs::read_to_string(&script).expect("script should be readable");
    let zshrc_content = fs::read_to_string(&zshrc).expect("zshrc should be readable");

    assert!(script_content.contains("wo()"));
    assert!(script_content.contains("--machine"));
    assert!(zshrc_content.contains("[ -f "));
    assert!(zshrc_content.contains("wo.zsh"));
}

#[test]
fn installed_prod_shell_function_switches_to_home_root() {
    let home = temp_root("installed_prod_shell_function_switches_to_home_root_home");
    let cwd = temp_root("installed_prod_shell_function_switches_to_home_root_cwd");
    let script = install_prod_shell(home.path());
    let expected_work = home.path().join(".workon/work/prod-hook-home-smoke");

    let output = Command::new("zsh")
        .arg("-f")
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
        .expect("zsh should run");

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
fn installed_dev_shell_function_switches_current_shell() {
    let home = temp_root("installed_dev_shell_function_switches_current_shell_home");
    let root = temp_root("installed_dev_shell_function_switches_current_shell_root");
    let script = install_dev_shell(home.path());
    let expected_work = home.path().join(".workon/work/hook-navigation-smoke");

    let output = Command::new("zsh")
        .arg("-f")
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
        .expect("zsh should run");

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

    let output = Command::new("zsh")
        .arg("-f")
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
        .expect("zsh should run");

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

fn install_prod_shell(home: &Path) -> PathBuf {
    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .env("HOME", home)
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

    home.join(".workon/shell/zsh/wo.zsh")
}

fn install_dev_shell(home: &Path) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .env("HOME", home)
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

    home.join(".workon/shell/zsh/wo-dev.zsh")
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
    let mut path = std::env::temp_dir();
    path.push(format!("workon-cli-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("temp root should be created");
    TempRoot { path }
}

fn fake_repo_tools() -> TempRoot {
    let fake_bin = temp_root("fake_repo_tools");
    fs::write(fake_bin.path().join("gh"), fake_gh_script()).expect("gh fake should be written");
    fs::write(fake_bin.path().join("git"), fake_git_script()).expect("git fake should be written");
    fs::write(fake_bin.path().join("wt"), fake_wt_script()).expect("wt fake should be written");

    for name in ["gh", "git", "wt"] {
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

fn fake_git_script() -> &'static str {
    r#"#!/bin/sh
printf 'git %s\n' "$*" >> "$WORKON_FAKE_LOG"
exit 0
"#
}

fn fake_wt_script() -> &'static str {
    r#"#!/bin/sh
printf 'WORKTRUNK_WORKTREE_PATH=%s wt %s\n' "$WORKTRUNK_WORKTREE_PATH" "$*" >> "$WORKON_FAKE_LOG"
if [ "$3" = "remove" ]; then
  exit 0
fi
mkdir -p "$WORKTRUNK_WORKTREE_PATH"
printf '{"action":"created","branch":"%s","path":"%s","created_branch":true,"base_branch":"main"}\n' "$5" "$WORKTRUNK_WORKTREE_PATH"
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
