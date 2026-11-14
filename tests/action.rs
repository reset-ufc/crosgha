#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! Runs `action/entrypoint.sh` the way the composite Action does, against a local release
//! server that serves this build's binary in cargo-dist's archive layout.

mod common;

use common::{
    Api, TOKEN, TempDir, bin, broken_release, copy_tree, fixture, github_api, isolate_git, release,
    serve, temp_dir,
};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

struct Run {
    output: Output,
    outputs: String,
    summary: String,
    temp: PathBuf,
}

fn run_action(base: &str, inputs: &str, extra: &[(&str, &str)]) -> Run {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/repos/configured");
    run_in(&repo, base, inputs, extra)
}

/// Runs the entry point from the caller's checkout `repo`.
fn run_in(repo: &Path, base: &str, inputs: &str, extra: &[(&str, &str)]) -> Run {
    let temp = temp_dir("runner");
    let outputs = temp.join("outputs");
    let summary = temp.join("summary");
    let mut command = Command::new("bash");
    command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("action/entrypoint.sh"))
        .current_dir(repo)
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

/// `follow-actions: true` checks the actions the workflows call and needs the API.
#[test]
fn action_follows_called_actions_only_when_online() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    let (api, _) = github_api(Api {
        repositories: fixture("follow"),
        ..Api::default()
    });
    let workflows = fixture("follow/caller/.github/workflows");
    let inputs = workflows.to_str().unwrap();
    let online = [
        ("CROSGHA_FOLLOW_ACTIONS", "true"),
        ("CROSGHA_TOKEN", TOKEN),
        ("GH_HOST", api.as_str()),
    ];
    let followed = run_action(&base, inputs, &online);
    let log = String::from_utf8_lossy(&followed.output.stdout).into_owned();
    assert!(followed.output.status.success(), "{log}");
    let sarif: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(output_value(&followed.outputs, "output-file").unwrap()).unwrap(),
    )
    .unwrap();
    let results = sarif["runs"][0]["results"].as_array().unwrap();
    let injection = results
        .iter()
        .find(|result| {
            result["ruleId"] == "script-injection"
                && result["message"]["text"]
                    .as_str()
                    .unwrap()
                    .starts_with("fixture/inner@")
        })
        .unwrap_or_else(|| panic!("{sarif:#}"));
    let location = &injection["locations"][0]["physicalLocation"];
    assert!(
        location["artifactLocation"]["uri"]
            .as_str()
            .unwrap()
            .ends_with("follow/caller/.github/workflows/ci.yml"),
        "{location}"
    );
    assert_eq!(location["region"]["startLine"], 12, "{location}");

    let offline = run_action(
        &base,
        inputs,
        &[
            ("CROSGHA_FOLLOW_ACTIONS", "true"),
            ("CROSGHA_ONLINE_AUDITS", "false"),
            ("CROSGHA_TOKEN", TOKEN),
        ],
    );
    assert!(!offline.output.status.success());
    assert!(
        String::from_utf8_lossy(&offline.output.stdout)
            .contains("::error::follow-actions requires online-audits: true and a token")
    );
    for run in [followed, offline] {
        fs::remove_dir_all(&run.temp).unwrap();
    }
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
    assert_eq!(
        format!("{}\n", body.join("\n")),
        report,
        "{}",
        plain.summary
    );
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

#[test]
fn job_summary_is_plain_markdown_in_every_mode() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());

    // Annotations go to the log; the summary keeps the count and no workflow commands.
    let annotated = run_action(
        &base,
        ".",
        &[
            ("CROSGHA_ANNOTATIONS", "true"),
            ("CROSGHA_ADVANCED_SECURITY", "false"),
        ],
    );
    assert!(annotated.output.status.success());
    let findings = output_value(&annotated.outputs, "findings").unwrap();
    assert_ne!(findings, "0", "{}", annotated.outputs);
    assert_eq!(
        annotated.summary,
        format!(
            "### crosgha v{}: {findings} finding(s)\n\nFindings are annotations in this run's log.\n",
            env!("CARGO_PKG_VERSION")
        )
    );

    // A caller's FORCE_COLOR reaches neither the summary nor the log.
    for extra in [
        vec![("FORCE_COLOR", "1")],
        vec![
            ("FORCE_COLOR", "1"),
            ("CROSGHA_ANNOTATIONS", "true"),
            ("CROSGHA_ADVANCED_SECURITY", "false"),
        ],
    ] {
        let run = run_action(&base, ".", &extra);
        let stdout = String::from_utf8_lossy(&run.output.stdout).into_owned();
        assert!(run.output.status.success(), "{stdout}");
        assert!(!run.summary.contains('\u{1b}'), "{:?}", run.summary);
        assert!(!stdout.contains('\u{1b}'), "{stdout:?}");
    }

    // The default mode still puts the text report in the summary.
    let plain = run_action(&base, ".", &[("FORCE_COLOR", "1")]);
    assert!(plain.summary.contains("```text\n"), "{}", plain.summary);
    assert!(
        plain.summary.contains("error[persist-credentials]"),
        "{}",
        plain.summary
    );
    assert!(!plain.summary.contains("::warning"), "{}", plain.summary);
}

/// The fix token of pull-request mode; no log, output, or summary may show it.
const FIX_TOKEN: &str = "fix-token-secret";

/// One request to the GitHub API stand-in.
struct Request {
    line: String,
    authorization: String,
    body: String,
}

/// An answer of the GitHub API stand-in: the method and path it answers, its status line, and its
/// body.
type Answer = (&'static str, &'static str, &'static str);

const NOT_FOUND: (&str, &str) = ("404 Not Found", r#"{"message":"Not Found"}"#);

/// A stand-in for the GitHub API that pull-request mode calls: it records each request and gives
/// the matching answer, or 404.
fn github_stand_in(answers: &'static [Answer]) -> (String, Arc<Mutex<Vec<Request>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::clone(&log);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut authorization = String::new();
            let mut length = 0;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
                let (name, value) = header.split_once(':').unwrap_or_default();
                match name.to_ascii_lowercase().as_str() {
                    "authorization" => authorization = value.trim().to_owned(),
                    "content-length" => length = value.trim().parse().unwrap(),
                    _ => {}
                }
            }
            let mut content = vec![0; length];
            reader.read_exact(&mut content).unwrap();
            let mut words = line.split_whitespace();
            let request = format!("{} {}", words.next().unwrap(), words.next().unwrap());
            let (status, body) = answers
                .iter()
                .find(|(answered, ..)| *answered == request)
                .map_or(NOT_FOUND, |(_, status, body)| (*status, *body));
            requests.lock().unwrap().push(Request {
                line: request,
                authorization,
                body: String::from_utf8(content).unwrap(),
            });
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (format!("http://{address}"), log)
}

fn git(directory: &Path, args: &[&str]) -> String {
    let output = isolate_git(&mut Command::new("git"))
        .current_dir(directory)
        .args([
            "-c",
            "user.name=Caller",
            "-c",
            "user.email=caller@example.com",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// A caller's checkout of `main` holding `files`, pushed to the bare repository `owner/repo.git`
/// under the second directory, which `GITHUB_SERVER_URL` names as a `file://` URL.
fn checkout(files: &[(&str, &str)]) -> (TempDir, TempDir) {
    let work = TempDir::new("checkout");
    let server = TempDir::new("server");
    let bare = server.join("owner/repo.git");
    fs::create_dir_all(&bare).unwrap();
    git(
        &bare,
        &["init", "--quiet", "--bare", "--initial-branch=main"],
    );
    git(&work, &["init", "--quiet", "--initial-branch=main"]);
    for (path, text) in files {
        let path = work.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    git(&work, &["add", "--all"]);
    git(&work, &["commit", "--quiet", "--message", "Initial commit"]);
    git(&work, &["push", "--quiet", bare.to_str().unwrap(), "main"]);
    (work, server)
}

/// The variables of a pull-request run on a push to `main`, besides the inputs.
fn pull_request_env(server: &Path, api: &str) -> Vec<(&'static str, String)> {
    vec![
        ("CROSGHA_FIX", "pull-request".to_owned()),
        ("CROSGHA_FIX_TOKEN", FIX_TOKEN.to_owned()),
        ("GITHUB_SERVER_URL", format!("file://{}", server.display())),
        ("GITHUB_API_URL", api.to_owned()),
        ("GITHUB_REPOSITORY", "owner/repo".to_owned()),
        ("GITHUB_REF", "refs/heads/main".to_owned()),
        ("GITHUB_RUN_ID", "42".to_owned()),
    ]
}

fn borrowed<'a>(env: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    env.iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect()
}

fn assert_hidden(run: &Run) {
    for text in [
        String::from_utf8_lossy(&run.output.stdout).into_owned(),
        String::from_utf8_lossy(&run.output.stderr).into_owned(),
        run.outputs.clone(),
        run.summary.clone(),
    ] {
        assert!(!text.contains(FIX_TOKEN), "{text}");
    }
}

#[test]
fn pull_request_mode_commits_only_the_fixes_and_opens_a_pull_request_listing_them() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    let ci = fs::read_to_string(fixture("checkout-credentials.before.yml")).unwrap();
    let mixed = fs::read_to_string(fixture("mixed-fixes.before.yml")).unwrap();
    let (work, server) = checkout(&[
        (".github/workflows/ci.yml", &ci),
        (".github/workflows/mixed.yml", &mixed),
        ("notes.txt", "draft\n"),
    ]);
    // What earlier steps of the caller's job changed stays out of the fix commit.
    fs::write(work.join("notes.txt"), "edited\n").unwrap();
    fs::write(work.join("build.log"), "log\n").unwrap();
    let main = git(&work, &["rev-parse", "HEAD"]);

    // The pull request must hold exactly what `crosgha --fix` writes on its own.
    let expected = TempDir::new("expected");
    copy_tree(&work.join(".github"), &expected.join(".github"));
    let fixed = bin()
        .current_dir(&*expected)
        .args([
            "--offline",
            "--persona",
            "pedantic",
            "--fix",
            "--format",
            "json",
            ".",
        ])
        .output()
        .unwrap();
    let report: serde_json::Value = serde_json::from_slice(&fixed.stdout).unwrap();
    let applied: Vec<&serde_json::Value> = report["fixes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fix| fix["status"] == "applied")
        .collect();
    assert!(applied.len() > 1, "{report}");

    let (api, requests) = github_stand_in(&[(
        "POST /repos/owner/repo/pulls",
        "201 Created",
        r#"{"number":7,"node_id":"PR_7","html_url":"https://github.com/owner/repo/pull/7"}"#,
    )]);
    let env = pull_request_env(&server, &api);
    let run = run_in(&work, &base, ".", &borrowed(&env));
    let stdout = String::from_utf8_lossy(&run.output.stdout).into_owned();
    assert!(run.output.status.success(), "{stdout}");
    assert_hidden(&run);
    // The findings of the scanned branch still apply `fail-on` after the upload.
    assert_eq!(output_value(&run.outputs, "exit-code"), Some("1"));
    assert_eq!(
        output_value(&run.outputs, "pull-request-url"),
        Some("https://github.com/owner/repo/pull/7"),
        "{}",
        run.outputs
    );
    assert!(
        run.summary.contains("https://github.com/owner/repo/pull/7"),
        "{}",
        run.summary
    );

    let bare = server.join("owner/repo.git");
    let branch = "crosgha/fix-42";
    assert_eq!(git(&bare, &["rev-parse", &format!("{branch}^")]), main);
    assert_eq!(
        git(&bare, &["diff", "--name-only", "main", branch]),
        ".github/workflows/ci.yml\n.github/workflows/mixed.yml\n"
    );
    for file in ["ci.yml", "mixed.yml"] {
        assert_eq!(
            git(
                &bare,
                &["show", &format!("{branch}:.github/workflows/{file}")]
            ),
            fs::read_to_string(expected.join(".github/workflows").join(file)).unwrap(),
            "{file}"
        );
    }
    assert_eq!(
        fs::read_to_string(work.join("notes.txt")).unwrap(),
        "edited\n"
    );
    assert!(work.join("build.log").is_file());

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.line, "POST /repos/owner/repo/pulls");
    assert_eq!(request.authorization, format!("Bearer {FIX_TOKEN}"));
    let pull: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(pull["head"], branch);
    assert_eq!(pull["base"], "main");
    assert!(!pull["title"].as_str().unwrap().is_empty());
    let body = pull["body"].as_str().unwrap();
    for fix in applied {
        let place = format!("{}:{}", fix["path"].as_str().unwrap(), fix["line"]);
        let rule = fix["rule"].as_str().unwrap();
        assert!(
            body.lines()
                .any(|line| line.contains(&place) && line.contains(rule)),
            "{place} {rule}:\n{body}"
        );
    }

    fs::remove_dir_all(&run.temp).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pull_request_mode_opens_nothing_without_a_fix_a_branch_or_an_accepted_push() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    let (api, requests) = github_stand_in(&[(
        "POST /repos/owner/repo/pulls",
        "422 Unprocessable Entity",
        r#"{"message":"Validation Failed"}"#,
    )]);
    let mut runs = Vec::new();

    // Findings whose only fix validation refuses leave nothing to propose.
    let unfixable = "name: Rejected fix\non: [push]\npermissions: { contents: read }\njobs:\n  lint:\n    runs-on: ubuntu-latest\n    timeout-minutes: 10\n    steps:\n      - { uses: actions/checkout@fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09 }\n      - run: make lint\n";
    let (work, server) = checkout(&[(".github/workflows/ci.yml", unfixable)]);
    let env = pull_request_env(&server, &api);
    let run = run_in(&work, &base, ".", &borrowed(&env));
    assert!(
        run.output.status.success(),
        "{}",
        String::from_utf8_lossy(&run.output.stdout)
    );
    assert_eq!(output_value(&run.outputs, "exit-code"), Some("1"));
    assert_eq!(output_value(&run.outputs, "pull-request-url"), None);
    let bare = server.join("owner/repo.git");
    assert_eq!(git(&bare, &["branch", "--list", "crosgha/*"]), "");
    runs.push(run);

    let ci = fs::read_to_string(fixture("checkout-credentials.before.yml")).unwrap();
    let (work, server) = checkout(&[(".github/workflows/ci.yml", &ci)]);
    let bare = server.join("owner/repo.git");

    // A pull request event checks out a merge commit, which has no branch to propose against.
    // Nothing listens on port 9, so the check must come before the download.
    let mut env = pull_request_env(&server, &api);
    env.push(("GITHUB_REF", "refs/pull/3/merge".to_owned()));
    let run = run_in(&work, "http://127.0.0.1:9", ".", &borrowed(&env));
    assert!(!run.output.status.success());
    assert!(
        String::from_utf8_lossy(&run.output.stdout).contains("branch"),
        "{}",
        String::from_utf8_lossy(&run.output.stdout)
    );
    runs.push(run);

    // The fix token stays required.
    let mut env = pull_request_env(&server, &api);
    env.push(("CROSGHA_FIX_TOKEN", String::new()));
    let run = run_in(&work, "http://127.0.0.1:9", ".", &borrowed(&env));
    assert!(!run.output.status.success());
    assert!(String::from_utf8_lossy(&run.output.stdout).contains("fix-token"));
    runs.push(run);

    // An earlier step's edit of a file crosgha then fixes would ride along in the fix commit.
    fs::write(
        work.join(".github/workflows/ci.yml"),
        format!("{ci}# edited by the build\n"),
    )
    .unwrap();
    let env = pull_request_env(&server, &api);
    let run = run_in(&work, &base, ".", &borrowed(&env));
    let stdout = String::from_utf8_lossy(&run.output.stdout).into_owned();
    assert!(!run.output.status.success());
    assert!(stdout.contains("already differed from HEAD"), "{stdout}");
    assert!(stdout.contains(".github/workflows/ci.yml"), "{stdout}");
    assert_eq!(git(&bare, &["branch", "--list", "crosgha/*"]), "");
    runs.push(run);
    git(&work, &["checkout", "--quiet", "--", "."]);

    // A branch left by an earlier attempt of this run refuses the push, before any pull request.
    git(&work, &["switch", "--quiet", "--create", "stale"]);
    fs::write(work.join("stale.txt"), "stale\n").unwrap();
    git(&work, &["add", "stale.txt"]);
    git(&work, &["commit", "--quiet", "--message", "Stale fix"]);
    git(
        &work,
        &[
            "push",
            "--quiet",
            bare.to_str().unwrap(),
            "stale:refs/heads/crosgha/fix-42",
        ],
    );
    git(&work, &["switch", "--quiet", "main"]);
    let env = pull_request_env(&server, &api);
    let run = run_in(&work, &base, ".", &borrowed(&env));
    let stdout = String::from_utf8_lossy(&run.output.stdout).into_owned();
    assert!(!run.output.status.success());
    assert!(stdout.contains("cannot push crosgha/fix-42"), "{stdout}");
    runs.push(run);
    assert_eq!(requests.lock().unwrap().len(), 0);

    // GitHub's refusal reaches the log with its message.
    git(
        &bare,
        &["branch", "--quiet", "--delete", "--force", "crosgha/fix-42"],
    );
    git(&work, &["checkout", "--quiet", "--", "."]);
    git(&work, &["switch", "--quiet", "main"]);
    let run = run_in(&work, &base, ".", &borrowed(&env));
    let stdout = String::from_utf8_lossy(&run.output.stdout).into_owned();
    assert!(!run.output.status.success());
    assert!(stdout.contains("Validation Failed"), "{stdout}");
    assert_eq!(requests.lock().unwrap().len(), 1);
    runs.push(run);

    for run in runs {
        assert_hidden(&run);
        fs::remove_dir_all(&run.temp).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}

/// A ruleset on `main` that requires the `test` check.
const REQUIRED_BY_RULESET: Answer = (
    "GET /repos/owner/repo/rules/branches/main",
    "200 OK",
    r#"[{"type":"required_status_checks","parameters":{"required_status_checks":[{"context":"test"}],"strict_required_status_checks_policy":false}}]"#,
);
/// Classic branch protection on `main` that requires the `ci` check.
const REQUIRED_BY_PROTECTION: Answer = (
    "GET /repos/owner/repo/branches/main",
    "200 OK",
    r#"{"name":"main","protected":true,"protection":{"enabled":true,"required_status_checks":{"enforcement_level":"non_admins","contexts":["ci"],"checks":[{"context":"ci","app_id":null}]}}}"#,
);
const NO_RULES: Answer = ("GET /repos/owner/repo/rules/branches/main", "200 OK", "[]");
const UNPROTECTED: Answer = (
    "GET /repos/owner/repo/branches/main",
    "200 OK",
    r#"{"name":"main","protected":false}"#,
);
const OPENED: Answer = (
    "POST /repos/owner/repo/pulls",
    "201 Created",
    r#"{"number":7,"node_id":"PR_7","html_url":"https://github.com/owner/repo/pull/7"}"#,
);
const ENABLED: Answer = (
    "POST /graphql",
    "200 OK",
    r#"{"data":{"enablePullRequestAutoMerge":{"pullRequest":{"url":"https://github.com/owner/repo/pull/7"}}}}"#,
);
const AUTO_MERGE_WARNING: &str = "`dangerously-auto-merge` is on";

fn auto_merge_env(server: &Path, api: &str) -> Vec<(&'static str, String)> {
    let mut env = pull_request_env(server, api);
    env.push(("CROSGHA_AUTO_MERGE", "true".to_owned()));
    env
}

#[test]
fn auto_merge_mode_enables_auto_merge_on_the_pull_request_it_opened() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    let ci = fs::read_to_string(fixture("checkout-credentials.before.yml")).unwrap();
    let mut runs = Vec::new();

    // Required checks come from a ruleset or from classic branch protection.
    let (by_ruleset, by_ruleset_log) =
        github_stand_in(&[REQUIRED_BY_RULESET, UNPROTECTED, OPENED, ENABLED]);
    let (by_protection, by_protection_log) =
        github_stand_in(&[NO_RULES, REQUIRED_BY_PROTECTION, OPENED, ENABLED]);
    for (api, log) in [
        (by_ruleset, by_ruleset_log),
        (by_protection, by_protection_log),
    ] {
        let (work, server) = checkout(&[(".github/workflows/ci.yml", &ci)]);
        let env = auto_merge_env(&server, &api);
        let run = run_in(&work, &base, ".", &borrowed(&env));
        let stdout = String::from_utf8_lossy(&run.output.stdout).into_owned();
        assert!(run.output.status.success(), "{stdout}");
        assert_eq!(
            output_value(&run.outputs, "pull-request-url"),
            Some("https://github.com/owner/repo/pull/7")
        );
        assert!(run.summary.contains(AUTO_MERGE_WARNING), "{}", run.summary);

        let requests = log.lock().unwrap();
        assert!(
            requests
                .iter()
                .all(|request| request.authorization == format!("Bearer {FIX_TOKEN}")),
        );
        // The Action only turns auto-merge on; GitHub merges once the checks pass.
        let enable: Vec<&Request> = requests
            .iter()
            .filter(|request| request.line == "POST /graphql")
            .collect();
        assert_eq!(enable.len(), 1);
        let mutation: serde_json::Value = serde_json::from_str(&enable[0].body).unwrap();
        assert!(
            mutation["query"]
                .as_str()
                .unwrap()
                .contains("enablePullRequestAutoMerge"),
            "{mutation}"
        );
        assert_eq!(mutation["variables"]["id"], "PR_7");
        assert!(
            !requests
                .iter()
                .any(|request| request.line.ends_with("/merge")),
            "{:?}",
            requests
                .iter()
                .map(|request| &request.line)
                .collect::<Vec<_>>()
        );
        drop(requests);
        runs.push(run);
    }

    for run in runs {
        assert_hidden(&run);
        fs::remove_dir_all(&run.temp).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn auto_merge_mode_fails_fast_without_required_status_checks_or_when_github_refuses() {
    let root = release(env!("CARGO_PKG_VERSION"), false);
    let base = serve(root.clone());
    let ci = fs::read_to_string(fixture("checkout-credentials.before.yml")).unwrap();
    let mut runs = Vec::new();

    // No required check on the base branch: nothing is pushed or opened.
    let (api, requests) = github_stand_in(&[NO_RULES, UNPROTECTED, OPENED, ENABLED]);
    let (work, server) = checkout(&[(".github/workflows/ci.yml", &ci)]);
    let env = auto_merge_env(&server, &api);
    let run = run_in(&work, &base, ".", &borrowed(&env));
    let stdout = String::from_utf8_lossy(&run.output.stdout).into_owned();
    assert!(!run.output.status.success());
    assert!(stdout.contains("no required status checks"), "{stdout}");
    assert!(run.summary.contains(AUTO_MERGE_WARNING), "{}", run.summary);
    assert_eq!(
        git(
            &server.join("owner/repo.git"),
            &["branch", "--list", "crosgha/*"]
        ),
        ""
    );
    assert!(
        requests
            .lock()
            .unwrap()
            .iter()
            .all(|request| request.line.starts_with("GET ")),
    );
    runs.push(run);

    // The warning comes first, so a run that ends early still shows it.
    let empty = TempDir::new("empty");
    let mut env = auto_merge_env(&server, &api);
    env.push(("CROSGHA_FAIL_ON_NO_INPUTS", "false".to_owned()));
    let run = run_in(&work, &base, empty.to_str().unwrap(), &borrowed(&env));
    assert!(run.output.status.success());
    assert_eq!(output_value(&run.outputs, "findings"), Some("0"));
    assert!(
        run.summary.contains(AUTO_MERGE_WARNING),
        "{:?}",
        run.summary
    );
    runs.push(run);

    // Branch rules the token cannot read leave the checks unknown, which is not a yes.
    let (api, _) = github_stand_in(&[
        (
            "GET /repos/owner/repo/rules/branches/main",
            "403 Forbidden",
            r#"{"message":"Resource not accessible by integration"}"#,
        ),
        UNPROTECTED,
        OPENED,
        ENABLED,
    ]);
    let (work, server) = checkout(&[(".github/workflows/ci.yml", &ci)]);
    let env = auto_merge_env(&server, &api);
    let run = run_in(&work, &base, ".", &borrowed(&env));
    let stdout = String::from_utf8_lossy(&run.output.stdout).into_owned();
    assert!(!run.output.status.success());
    assert!(
        stdout.contains("Resource not accessible by integration"),
        "{stdout}"
    );
    runs.push(run);

    // GitHub refusing auto-merge fails the step and names the pull request it left open.
    let (api, _) = github_stand_in(&[
        REQUIRED_BY_RULESET,
        UNPROTECTED,
        OPENED,
        (
            "POST /graphql",
            "200 OK",
            r#"{"data":null,"errors":[{"message":"Auto merge is not allowed for this repository"}]}"#,
        ),
    ]);
    let (work, server) = checkout(&[(".github/workflows/ci.yml", &ci)]);
    let env = auto_merge_env(&server, &api);
    let run = run_in(&work, &base, ".", &borrowed(&env));
    let stdout = String::from_utf8_lossy(&run.output.stdout).into_owned();
    assert!(!run.output.status.success());
    assert!(
        stdout.contains("Auto merge is not allowed for this repository")
            && stdout.contains("https://github.com/owner/repo/pull/7"),
        "{stdout}"
    );
    runs.push(run);

    for run in runs {
        assert_hidden(&run);
        fs::remove_dir_all(&run.temp).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}
