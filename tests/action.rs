#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! Runs `action/entrypoint.sh` the way the composite Action does, against a local release
//! server that serves this build's binary in cargo-dist's archive layout.

mod common;

use common::{broken_release, release, serve, temp_dir};
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
        .env("RUNNER_ARCH", "X64")
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

/// The workflow command lines the runner would act on in `log`, honoring `::stop-commands::`.
fn live_commands(log: &str) -> Vec<&str> {
    let mut stopped: Option<String> = None;
    let mut live = Vec::new();
    for line in log.lines().map(str::trim) {
        match &stopped {
            Some(token) => {
                if line == format!("::{token}::") {
                    stopped = None;
                }
            }
            None => match line.strip_prefix("::stop-commands::") {
                Some(token) => stopped = Some(token.to_owned()),
                None if line.starts_with("::") => live.push(line),
                None => {}
            },
        }
    }
    live
}

fn sarif_uris(run: &Run) -> Vec<String> {
    let sarif: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(output_value(&run.outputs, "output-file").unwrap()).unwrap(),
    )
    .unwrap();
    sarif["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|result| {
            result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
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
    assert!(String::from_utf8_lossy(&run.output.stdout).contains("checksum"));
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

/// One empty input among others must not hide their findings: the Action reports the inputs that
/// have workflows and warns only about the empty one.
#[test]
fn action_tolerating_missing_inputs_still_reports_the_other_inputs() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    let empty = temp_dir("empty");
    let findings = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checkout-credentials.before.yml");
    let present = format!(".github/workflows/ci.yml\n{}\n", findings.display());
    let mixed = format!("{present}{}\n", empty.display());

    let alone = run_action(&base, &present, &[]);
    let expected = output_value(&alone.outputs, "findings").unwrap().to_owned();
    assert_ne!(expected, "0", "the fixture must report findings");

    let strict = run_action(&base, &mixed, &[]);
    assert!(!strict.output.status.success());

    let tolerant = run_action(&base, &mixed, &[("CROSGHA_FAIL_ON_NO_INPUTS", "false")]);
    let log = String::from_utf8_lossy(&tolerant.output.stdout).into_owned();
    assert!(tolerant.output.status.success(), "{log}");
    assert_eq!(
        output_value(&tolerant.outputs, "findings"),
        Some(expected.as_str())
    );
    assert_eq!(output_value(&tolerant.outputs, "exit-code"), Some("1"));
    let uris = sarif_uris(&tolerant);
    for file in ["ci.yml", "checkout-credentials.before.yml"] {
        assert!(
            uris.iter().any(|uri| uri.ends_with(file)),
            "{file}: {uris:?}"
        );
    }
    let warnings: Vec<&str> = live_commands(&log)
        .into_iter()
        .filter(|line| line.starts_with("::warning::"))
        .collect();
    assert_eq!(
        warnings,
        [format!(
            "::warning::crosgha found no workflow files in {}",
            empty.display()
        )],
        "{log}"
    );

    for run in [alone, strict, tolerant] {
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
        String::from_utf8_lossy(&run.output.stdout).contains("cannot download"),
        "{}",
        String::from_utf8_lossy(&run.output.stdout)
    );
    assert_eq!(run.outputs, "");
    fs::remove_dir_all(&run.temp).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn action_fails_without_outputs_when_crosgha_cannot_run_or_crashes() {
    // Nothing listens on port 9: the runner check must fail before any download.
    let arm = run_action("http://127.0.0.1:9", ".", &[("RUNNER_ARCH", "ARM64")]);
    assert!(!arm.output.status.success());
    assert!(String::from_utf8_lossy(&arm.output.stdout).contains("X64"));
    assert_eq!(arm.outputs, "");
    fs::remove_dir_all(&arm.temp).unwrap();

    for (code, message) in [
        (101, "thread 'main' panicked"),
        (126, "cannot execute binary file: Exec format error"),
    ] {
        let root = broken_release(
            env!("CARGO_PKG_VERSION"),
            &format!("#!/bin/sh\necho \"{message}\" >&2\nexit {code}\n"),
        );
        let base = serve(root.clone());
        let run = run_action(&base, ".", &[]);
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&run.output.stdout),
            String::from_utf8_lossy(&run.output.stderr)
        );
        assert!(!run.output.status.success(), "{output}");
        assert!(output.contains(message), "{output}");
        // The error shares stdout with the stop and resume markers, so it lands after the resume.
        let stdout = String::from_utf8_lossy(&run.output.stdout);
        assert_eq!(
            live_commands(&stdout),
            [
                format!("::error::crosgha could not complete the scan (exit status {code})")
                    .as_str()
            ],
            "{output}"
        );
        assert_eq!(run.outputs, "", "exit {code}");
        assert_eq!(run.summary, "", "exit {code}");
        fs::remove_dir_all(&run.temp).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn action_scans_every_path_of_a_multi_line_inputs_block() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    // `inputs: |` in the caller's workflow yields one path per line.
    let other = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checkout-credentials.before.yml");
    let run = run_action(
        &base,
        &format!(".github/workflows/ci.yml\n{}\n", other.display()),
        &[],
    );
    assert!(
        run.output.status.success(),
        "{}",
        String::from_utf8_lossy(&run.output.stderr)
    );
    let uris = sarif_uris(&run);
    for file in ["ci.yml", "checkout-credentials.before.yml"] {
        assert!(
            uris.iter().any(|uri| uri.ends_with(file)),
            "{file}: {uris:?}"
        );
    }
    fs::remove_dir_all(&run.temp).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn action_keeps_scanned_file_names_from_forging_commands_or_breaking_the_summary() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    // A pull request controls file names: this one forges an annotation and closes a ``` fence.
    let repo = temp_dir("hostile");
    fs::write(
        repo.join("w\n::warning title=forged::pwned\n````\n# heading\n.yml"),
        "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n",
    )
    .unwrap();
    let inputs = repo.to_str().unwrap();

    let plain = run_action(&base, inputs, &[]);
    let stdout = String::from_utf8_lossy(&plain.output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&plain.output.stderr).into_owned();
    assert!(plain.output.status.success(), "{stderr}");
    assert!(
        stdout.contains("\n::warning title=forged::pwned\n"),
        "{stdout}"
    );
    assert_eq!(live_commands(&stdout), Vec::<&str>::new(), "{stdout}");
    assert_eq!(live_commands(&stderr), Vec::<&str>::new(), "{stderr}");

    // The whole report sits inside one fence: no report line closes it early.
    let report = fs::read_to_string(plain.temp.join("crosgha.txt")).unwrap();
    let mut lines = plain
        .summary
        .lines()
        .skip_while(|line| !line.starts_with('`'));
    let fence = lines.next().unwrap().trim_end_matches("text").to_owned();
    let closes = |line: &str| {
        let line = line.trim();
        line.len() >= fence.len() && line.chars().all(|c| c == '`')
    };
    let body: Vec<&str> = lines.by_ref().take_while(|line| !closes(line)).collect();
    assert_eq!(body.join("\n") + "\n", report, "{}", plain.summary);
    assert_eq!(lines.next(), None, "{}", plain.summary);

    // Annotation mode keeps crosgha's own escaped commands live, and only those.
    let annotated = run_action(
        &base,
        inputs,
        &[
            ("CROSGHA_ANNOTATIONS", "true"),
            ("CROSGHA_ADVANCED_SECURITY", "false"),
        ],
    );
    let stdout = String::from_utf8_lossy(&annotated.output.stdout).into_owned();
    assert!(annotated.output.status.success());
    let live = live_commands(&stdout);
    assert!(!live.is_empty(), "{stdout}");
    assert!(
        live.iter()
            .all(|command| ["::error ", "::warning ", "::notice "]
                .iter()
                .any(|kind| command.starts_with(&format!("{kind}file=")))),
        "{live:#?}"
    );
    assert_eq!(
        live_commands(&String::from_utf8_lossy(&annotated.output.stderr)),
        Vec::<&str>::new()
    );

    for run in [plain, annotated] {
        fs::remove_dir_all(&run.temp).unwrap();
    }
    fs::remove_dir_all(repo).unwrap();
    fs::remove_dir_all(root).unwrap();
}
