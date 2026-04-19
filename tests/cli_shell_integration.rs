use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn machine_mode_emits_cd_target_for_created_work() {
    let root = temp_root("machine_mode_emits_cd_target_for_created_work");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
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
fn installed_dev_shell_function_switches_current_shell() {
    let home = temp_root("installed_dev_shell_function_switches_current_shell_home");
    let root = temp_root("installed_dev_shell_function_switches_current_shell_root");
    let script = install_dev_shell(home.path());

    let output = Command::new("zsh")
        .arg("-f")
        .arg("-c")
        .arg(format!(
            "source {}; cd {}; wo --intent investigate 'Hook navigation smoke'; printf 'PWD:%s\\n' \"$PWD\"; test \"${{PWD##*/}}\" = hook-navigation-smoke",
            shell_quote(&script),
            shell_quote(root.path())
        ))
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
}

#[test]
fn installed_dev_shell_intercepts_just_wo() {
    let home = temp_root("installed_dev_shell_intercepts_just_wo_home");
    let root = temp_root("installed_dev_shell_intercepts_just_wo_root");
    let script = install_dev_shell(home.path());

    let output = Command::new("zsh")
        .arg("-f")
        .arg("-c")
        .arg(format!(
            "source {}; export WORKON_DEV_ROOT={}; cd {}; just wo --intent investigate 'Just hook navigation smoke'; printf 'PWD:%s\\n' \"$PWD\"; test \"${{PWD##*/}}\" = just-hook-navigation-smoke",
            shell_quote(&script),
            shell_quote(root.path()),
            shell_quote(root.path())
        ))
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

fn shell_quote(path: &Path) -> String {
    let value = path.display().to_string();
    format!("'{}'", value.replace('\'', "'\\''"))
}
