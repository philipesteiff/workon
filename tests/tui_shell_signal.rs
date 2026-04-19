use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn machine_switch_writes_cd_signal_to_signal_file_when_requested() {
    let root = temp_root("machine_switch_writes_cd_signal_to_signal_file_when_requested");
    let signal_file = root.path().join("signals.txt");

    let create_output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .args(["--intent", "investigate", "Billing retry audit"])
        .output()
        .expect("wo should run");
    assert!(create_output.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_wo"))
        .current_dir(root.path())
        .env("WORKON_SIGNAL_FILE", &signal_file)
        .args(["--machine", "billing-retry-audit"])
        .output()
        .expect("wo should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let signals = fs::read_to_string(&signal_file).expect("signal file should be written");

    assert!(stdout.contains("work opened: Billing retry audit"));
    assert!(!stdout.contains("__WORKON_CD="));
    assert!(signals.contains("__WORKON_CD="));
    assert!(signals.contains("__WORKON_ROOT="));
    assert!(signals.contains("__WORKON_TITLE=Billing retry audit"));
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
    path.push(format!(
        "workon-tui-shell-test-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("temp root should be created");
    TempRoot { path }
}
