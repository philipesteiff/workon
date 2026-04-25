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
