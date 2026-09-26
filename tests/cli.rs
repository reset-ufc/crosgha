use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_our-tool"))
}

#[test]
fn prints_version() {
    let output = bin()
        .arg("--version")
        .output()
        .expect("failed to run binary");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.trim(),
        format!("our-tool {}", env!("CARGO_PKG_VERSION"))
    );
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn scans_workflow_directory() {
    let output = bin()
        .arg(fixture("workflows"))
        .output()
        .expect("failed to run binary");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("YAML parsed; `on` and `jobs` found"));
    assert!(stdout.contains("Checked 2 workflow file(s)"));
    assert!(stdout.contains("smell detection and fixes are not implemented yet."));
}

#[test]
fn rejects_malformed_yaml() {
    let output = bin()
        .arg(fixture("malformed.yml"))
        .output()
        .expect("failed to run binary");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("input is not valid YAML"));
    assert!(stderr.contains("malformed.yml"));
}

#[test]
fn rejects_non_workflow_yaml() {
    let output = bin()
        .arg(fixture("missing-workflow-keys.yml"))
        .output()
        .expect("failed to run binary");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing required top-level key `on`"));
    assert!(stderr.contains("missing-workflow-keys.yml"));
}
