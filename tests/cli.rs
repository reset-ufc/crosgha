use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_crosgha"))
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
        format!("crosgha {}", env!("CARGO_PKG_VERSION"))
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
    assert!(stdout.contains("0 finding(s)"));
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

#[test]
fn reports_checkout_credentials_across_jobs() {
    let output = bin()
        .arg(fixture("checkout-credentials.before.yml"))
        .output()
        .expect("failed to run binary");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches("persist-credentials").count(), 3);
    assert!(stdout.contains("3 finding(s)"));
}

#[test]
fn fixes_checkout_credentials_without_changing_other_nodes() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("crosgha-{}-{unique}.yml", std::process::id()));
    fs::copy(fixture("checkout-credentials.before.yml"), &path).unwrap();

    let output = bin().arg("--fix").arg(&path).output().unwrap();
    let actual = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        actual,
        fs::read_to_string(fixture("checkout-credentials.after.yml")).unwrap()
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Fixed 3 finding(s)")
    );

    let checked = bin()
        .arg(fixture("checkout-credentials.after.yml"))
        .output()
        .unwrap();
    assert!(checked.status.success());
}

#[test]
fn fix_does_not_overwrite_invalid_workflow() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "crosgha-invalid-{}-{unique}.yml",
        std::process::id()
    ));
    fs::copy(fixture("malformed.yml"), &path).unwrap();
    let before = fs::read(&path).unwrap();

    let output = bin().arg("--fix").arg(&path).output().unwrap();
    let after = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();

    assert!(!output.status.success());
    assert_eq!(before, after);
}

#[test]
fn unavailable_report_format_fails_before_fixing() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "crosgha-format-{}-{unique}.yml",
        std::process::id()
    ));
    fs::copy(fixture("checkout-credentials.before.yml"), &path).unwrap();
    let before = fs::read(&path).unwrap();

    let output = bin()
        .arg("--format")
        .arg("sarif")
        .arg("--fix")
        .arg(&path)
        .output()
        .unwrap();
    let after = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("not implemented")
    );
    assert_eq!(before, after);
}

#[test]
fn failure_threshold_changes_exit_code_without_hiding_findings() {
    let workflow = fixture("checkout-credentials.before.yml");
    let error_only = bin()
        .args(["--fail-on", "error"])
        .arg(&workflow)
        .output()
        .unwrap();
    assert!(error_only.status.success());
    assert!(
        String::from_utf8(error_only.stdout)
            .unwrap()
            .contains("3 finding(s)")
    );

    let warning = bin()
        .args(["--fail-on", "warning"])
        .arg(workflow)
        .output()
        .unwrap();
    assert_eq!(warning.status.code(), Some(1));
    assert!(
        String::from_utf8(warning.stdout)
            .unwrap()
            .contains("3 finding(s)")
    );
}
