#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! End-to-end runs of the built binary: inputs, discovery, exit status, and flags.

mod common;

use common::{TempDir, bin, fixture};
use std::fs;

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

#[test]
fn scans_workflow_directory() {
    let output = bin()
        .arg(fixture("workflows"))
        .output()
        .expect("failed to run binary");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "✓ Checked 2 workflow files: no findings.\n");
}

#[test]
fn rejects_malformed_yaml() {
    let output = bin()
        .arg(fixture("malformed.yml"))
        .output()
        .expect("failed to run binary");
    assert_eq!(output.status.code(), Some(2));
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
    assert_eq!(output.status.code(), Some(2));
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
    assert_eq!(stdout.matches("[persist-credentials]").count(), 3);
    assert!(stdout.contains("3 findings"));
}

#[test]
fn invalid_workflow_does_not_hide_findings_in_the_others() {
    let temp = TempDir::new("mixed");
    let dir = temp.join("work");
    fs::create_dir(&dir).unwrap();
    let valid = dir.join("ci.yml");
    fs::copy(fixture("checkout-credentials.before.yml"), &valid).unwrap();
    fs::copy(fixture("malformed.yml"), dir.join("broken.yml")).unwrap();
    let before = fs::read(&valid).unwrap();

    let report = bin().arg(&dir).output().unwrap();
    let json = bin().args(["-f", "json"]).arg(&dir).output().unwrap();
    let sarif = bin().args(["-f", "sarif"]).arg(&dir).output().unwrap();
    let fix = bin().arg("--fix").arg(&dir).output().unwrap();
    let fix_json = bin()
        .args(["--fix", "-f", "json"])
        .arg(&dir)
        .output()
        .unwrap();
    let after = fs::read(&valid).unwrap();

    assert_eq!(report.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&report.stderr).contains("broken.yml"));
    let stdout = String::from_utf8(report.stdout).unwrap();
    assert_eq!(
        stdout.matches("[persist-credentials]").count(),
        3,
        "{stdout}"
    );

    // Machine-readable formats carry the failure; stderr is not enough for a SARIF consumer.
    assert_eq!(json.status.code(), Some(2));
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(json["summary"]["files"], 1, "{json}");
    assert_eq!(json["summary"]["errors"], 1, "{json}");
    assert_eq!(json["findings"].as_array().unwrap().len(), 3, "{json}");
    let error = &json["errors"][0];
    assert!(
        error["path"].as_str().unwrap().ends_with("broken.yml")
            && !error["message"].as_str().unwrap().is_empty(),
        "{error}"
    );
    assert_eq!(sarif.status.code(), Some(2));
    let sarif: serde_json::Value = serde_json::from_slice(&sarif.stdout).unwrap();
    let run = &sarif["runs"][0];
    assert_eq!(run["results"].as_array().unwrap().len(), 3, "{run}");
    let invocation = &run["invocations"][0];
    assert_eq!(invocation["executionSuccessful"], false, "{invocation}");
    let notification = &invocation["toolExecutionNotifications"][0];
    assert_eq!(notification["level"], "error", "{notification}");
    assert!(
        notification["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
            .as_str()
            .unwrap()
            .ends_with("broken.yml"),
        "{notification}"
    );

    // A failed input keeps the exit-2 promise that no file changes, but still shows the findings.
    assert_eq!(fix.status.code(), Some(2));
    assert_eq!(before, after);
    let stdout = String::from_utf8(fix.stdout).unwrap();
    assert_eq!(
        stdout.matches("warning[persist-credentials]").count(),
        3,
        "{stdout}"
    );
    assert_eq!(
        stdout
            .matches("[persist-credentials]: not written because a workflow failed to parse")
            .count(),
        3,
        "{stdout}"
    );
    assert!(!stdout.contains("Fixed"), "{stdout}");
    assert!(!stdout.contains("rerun with --fix"), "{stdout}");
    // The report never claims a fix that was not written.
    assert_eq!(fix_json.status.code(), Some(2));
    let fix_json: serde_json::Value = serde_json::from_slice(&fix_json.stdout).unwrap();
    assert_eq!(fix_json["summary"]["fixed"], 0, "{fix_json}");
    assert_eq!(fix_json["summary"]["findings"], 3, "{fix_json}");
    let fixes = fix_json["fixes"].as_array().unwrap();
    assert_eq!(fixes.len(), 3, "{fix_json}");
    assert!(
        fixes.iter().all(|fix| fix["status"] == "not-written"),
        "{fix_json}"
    );
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
            .contains("3 findings")
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
            .contains("3 findings")
    );
}

#[test]
fn no_path_the_repository_root_and_github_dir_scan_the_same_workflows() {
    // `.github` also holds `crosgha.yml` and `dependabot.yml`, which are not workflows.
    let scan = |paths: &[&str]| {
        let output = bin()
            .current_dir(fixture("repos/cooldown"))
            .args(["--persona", "auditor", "--offline", "--fail-on", "none"])
            .args(paths)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{paths:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().replace("./", "")
    };
    let default = scan(&[]);
    assert!(default.contains("Checked 1 workflow file:"), "{default}");
    assert!(default.contains(".github/dependabot.yml:3:"), "{default}");
    let absolute = fixture("repos/cooldown/.github/workflows/ci.yml");
    let absolute = absolute.to_str().unwrap();
    for paths in [&["."][..], &[".github"], &[".github/workflows"]] {
        assert_eq!(scan(paths), default, "{paths:?}");
    }
    // Overlapping inputs, relative or absolute, report each workflow and `.github` once.
    let overlapping = scan(&[".", ".github", ".github/workflows", absolute]);
    assert!(
        overlapping.contains("Checked 1 workflow file:"),
        "{overlapping}"
    );
    assert_eq!(
        overlapping.matches("[missing-release-cooldown]").count(),
        default.matches("[missing-release-cooldown]").count(),
        "{overlapping}"
    );
}

/// A report that cannot reach stdout is an error, not a panic: a full disk exits 2 with a
/// message, and a reader that closed the pipe early (`| head`) ends the run quietly.
#[cfg(target_os = "linux")]
#[test]
fn unwritable_stdout_is_reported_without_a_panic() {
    let full = |args: &[&str]| {
        let device = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        bin().args(args).stdout(device).output().unwrap()
    };
    for args in [
        &["--list-rules", "-f", "json"][..],
        &["--list-rules"][..],
        &[
            "--offline",
            "--no-config",
            "-f",
            "sarif",
            "--fail-on",
            "none",
        ][..],
    ] {
        let mut args = args.to_vec();
        let workflow = fixture("checkout-credentials.before.yml");
        if !args.contains(&"--list-rules") {
            args.push(workflow.to_str().unwrap());
        }
        let output = full(&args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains("cannot write the report"), "{stderr}");
        assert!(!stderr.contains("panicked"), "{stderr}");
    }

    let temp = TempDir::new("migrate");
    let repo = temp.join("work");
    fs::create_dir_all(repo.join(".github/workflows")).unwrap();
    fs::copy(
        fixture("checkout-credentials.before.yml"),
        repo.join(".github/workflows/ci.yml"),
    )
    .unwrap();
    let workflows = repo.join(".github/workflows");
    let output = full(&[
        "migrate",
        "--from",
        "zizmor",
        "--action",
        "owner/crosgha@0123456789abcdef0123456789abcdef01234567",
        "--action-version",
        "v0.1.0",
        workflows.to_str().unwrap(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "migrate: {stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");

    let mut child = bin()
        .args(["--list-rules", "-f", "json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
}

#[test]
fn repository_roots_and_several_inputs_are_collected_like_zizmor() {
    let root = bin()
        .args(["--offline", "--persona", "auditor", "--fail-on", "none"])
        .arg(fixture("repos/insecure"))
        .output()
        .unwrap();
    assert!(
        root.status.success(),
        "{}",
        String::from_utf8_lossy(&root.stderr)
    );
    let stdout = String::from_utf8(root.stdout).unwrap();
    assert!(stdout.contains("Checked 1 workflow file:"), "{stdout}");
    assert!(stdout.contains("[missing-release-cooldown]"), "{stdout}");

    let several = bin()
        .args(["--offline", "--fail-on", "none"])
        .arg(fixture("workflows"))
        .arg(fixture("repos/insecure/.github/workflows"))
        .output()
        .unwrap();
    assert!(
        several.status.success(),
        "{}",
        String::from_utf8_lossy(&several.stderr)
    );
    assert!(
        String::from_utf8(several.stdout)
            .unwrap()
            .contains("Checked 3 workflow files")
    );
}

#[test]
fn only_files_directly_in_the_workflows_directory_are_scanned() {
    // `config/labels.yml` is data read by a job; GitHub never loads it as a workflow.
    let nested = fixture("repos/nested");
    for input in [nested.clone(), nested.join(".github/workflows")] {
        let output = bin().arg("--offline").arg(&input).output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            output.status.success(),
            "{input:?}: {stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(stdout, "✓ Checked 1 workflow file: no findings.\n");
    }

    // A repository root without workflows is not searched for other YAML (action.yml, compose).
    let output = bin()
        .arg("--offline")
        .arg(fixture("repos/action-only"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("no .yml or .yaml workflow files found in")
            && stderr.contains(".github/workflows"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("missing required top-level key"),
        "{stderr}"
    );
}

#[test]
fn zizmor_pedantic_flag_and_force_color_work_as_in_zizmor() {
    let scan = |args: &[&str], env: &[(&str, &str)]| {
        let output = bin()
            .env_remove("NO_COLOR")
            .env_remove("CLICOLOR_FORCE")
            .envs(env.iter().copied())
            .args(["--offline", "--no-config", "--fail-on", "none"])
            .args(args)
            .arg(fixture("repos/configured/.github/workflows/ci.yml"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    // https://docs.zizmor.sh/usage/: `--pedantic` is an alias for `--persona=pedantic`.
    let pedantic = scan(&["--persona", "pedantic"], &[]);
    assert_ne!(pedantic, scan(&[], &[]));
    assert_eq!(scan(&["--pedantic"], &[]), pedantic);

    let colored = scan(&[], &[("FORCE_COLOR", "1")]);
    assert!(colored.contains('\x1b'), "{colored}");
    for env in [
        &[("FORCE_COLOR", "")][..],
        &[("FORCE_COLOR", "1"), ("NO_COLOR", "1")],
    ] {
        let plain = scan(&[], env);
        assert!(!plain.contains('\x1b'), "{env:?}: {plain}");
    }
}
