//! Runs `action/entrypoint.sh` the way the composite Action does, against a local release
//! server that serves this build's binary in cargo-dist's archive layout.

mod common;

use common::{release, serve, temp_dir};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Run {
    output: Output,
    outputs: String,
    summary: String,
    temp: PathBuf,
}

fn run_action(base: &str, inputs: &str, extra: &[(&str, &str)]) -> Run {
    let temp = temp_dir("runner");
    let outputs = temp.join("outputs");
    let summary = temp.join("summary");
    let mut command = Command::new("bash");
    command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("action/entrypoint.sh"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/repos/configured"))
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap())
        .env("RUNNER_OS", "Linux")
        .env("GITHUB_ACTIONS", "true")
        .env("RUNNER_TEMP", &temp)
        .env("GITHUB_OUTPUT", &outputs)
        .env("GITHUB_STEP_SUMMARY", &summary)
        .env("GITHUB_ACTION_PATH", env!("CARGO_MANIFEST_DIR"))
        .env("CROSGHA_RELEASE_BASE", base)
        .env("CROSGHA_INPUTS", inputs)
        .env("CROSGHA_PERSONA", "pedantic")
        .env("CROSGHA_ONLINE_AUDITS", "true")
        .env("CROSGHA_TOKEN", "")
        .env("CROSGHA_ADVANCED_SECURITY", "true")
        .env("CROSGHA_ANNOTATIONS", "false")
        .env("CROSGHA_FAIL_ON", "any")
        .env("CROSGHA_FIX", "none")
        .env("CROSGHA_AUTO_MERGE", "false")
        .env("CROSGHA_FAIL_ON_NO_INPUTS", "true");
    for (key, value) in extra {
        command.env(key, value);
    }
    let output = command.output().unwrap();
    Run {
        output,
        outputs: fs::read_to_string(outputs).unwrap_or_default(),
        summary: fs::read_to_string(summary).unwrap_or_default(),
        temp,
    }
}

fn output_value<'a>(outputs: &'a str, key: &str) -> Option<&'a str> {
    outputs
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
}

#[test]
fn action_installs_the_verified_release_and_reports_sarif() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    let run = run_action(&base, ".", &[]);
    let stderr = String::from_utf8_lossy(&run.output.stderr).into_owned();
    assert!(run.output.status.success(), "{stderr}");

    // Findings do not fail this step: the SARIF upload runs first, then the exit code applies.
    assert_eq!(
        output_value(&run.outputs, "exit-code"),
        Some("1"),
        "{}",
        run.outputs
    );
    let findings: usize = output_value(&run.outputs, "findings")
        .unwrap()
        .parse()
        .unwrap();
    assert!(findings > 0, "{}", run.outputs);
    let sarif_file = output_value(&run.outputs, "output-file").unwrap();
    let sarif: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sarif_file).unwrap()).unwrap();
    assert_eq!(
        sarif["runs"][0]["results"].as_array().unwrap().len(),
        findings
    );
    assert_eq!(
        sarif["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
            .as_str()
            .unwrap()
            .split('/')
            .next(),
        Some(".github")
    );
    assert!(run.summary.contains("crosgha"), "{}", run.summary);
    assert!(
        run.summary.contains("[script-injection]"),
        "{}",
        run.summary
    );
    // Online audits were requested without a token, so the run stays offline.
    assert!(!stderr.contains("GitHub API"), "{stderr}");
    fs::remove_dir_all(&run.temp).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn action_refuses_a_release_whose_checksum_does_not_match() {
    let root = release(env!("CARGO_PKG_VERSION"), true);
    let base = serve(root.clone());
    let run = run_action(&base, ".", &[]);
    assert!(!run.output.status.success());
    assert!(String::from_utf8_lossy(&run.output.stderr).contains("checksum"));
    assert_eq!(output_value(&run.outputs, "findings"), None);
    fs::remove_dir_all(&run.temp).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn action_can_tolerate_missing_inputs() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    let empty = temp_dir("empty");
    let empty = empty.to_str().unwrap();
    let strict = run_action(&base, empty, &[]);
    assert!(!strict.output.status.success());
    let tolerant = run_action(&base, empty, &[("CROSGHA_FAIL_ON_NO_INPUTS", "false")]);
    assert!(
        tolerant.output.status.success(),
        "{}",
        String::from_utf8_lossy(&tolerant.output.stderr)
    );
    assert_eq!(output_value(&tolerant.outputs, "findings"), Some("0"));
    for run in [strict, tolerant] {
        fs::remove_dir_all(&run.temp).unwrap();
    }
    fs::remove_dir_all(empty).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn action_uses_plain_http_only_for_a_bare_loopback_origin() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    // Userinfo makes the URL start like the test origin while naming another host.
    let disguised = base.replacen("http://", "http://127.0.0.1:1@", 1);
    let run = run_action(&disguised, ".", &[]);
    assert!(!run.output.status.success());
    assert!(
        String::from_utf8_lossy(&run.output.stderr).contains("cannot download"),
        "{}",
        String::from_utf8_lossy(&run.output.stderr)
    );
    assert_eq!(run.outputs, "");
    fs::remove_dir_all(&run.temp).unwrap();
    fs::remove_dir_all(root).unwrap();
}
