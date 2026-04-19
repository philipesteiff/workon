use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn machine_mode_emits_switch_target_for_created_work() {
    let root = temp_root("machine_mode_emits_switch_target_for_created_work");

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
    assert!(stdout.contains("__WORKON_SWITCH_PATH="));
    assert!(stdout.contains("__WORKON_SWITCH_TITLE=Answer technical question manager"));
}

#[test]
fn normal_create_enters_work_shell() {
    let root = temp_root("normal_create_enters_work_shell");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_SKIP_USER_ZSHRC", "1")
        .env(
            "WORKON_SHELL_COMMAND",
            "printf 'INSIDE:%s\\n' \"$PWD\"; test -f AGENTS.md; test -f CLAUDE.md",
        )
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
    assert!(stdout.contains("INSIDE:"));
    assert!(stdout.contains("answer-technical-question-manager"));
}

#[test]
fn injected_work_shell_function_switches_without_stacking_shells() {
    let root = temp_root("injected_work_shell_function_switches_without_stacking_shells");

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_SKIP_USER_ZSHRC", "1")
        .env(
            "WORKON_SHELL_COMMAND",
            "wo --intent investigate 'Switch target from work shell'; printf 'SWITCHED:%s\\n' \"$PWD\"; test \"${PWD##*/}\" = switch-target-from-work-shell",
        )
        .args(["--intent", "investigate", "Initial work shell"])
        .output()
        .expect("wo should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    assert!(stdout.contains("work created: Initial work shell"));
    assert!(stdout.contains("work created: Switch target from work shell"));
    assert!(stdout.contains("SWITCHED:"));
    assert!(stdout.contains("switch-target-from-work-shell"));
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
