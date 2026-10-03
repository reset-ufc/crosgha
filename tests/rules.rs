//! Data-driven checks for every rule. Each `<rule>.before.yml` fixture marks the lines that
//! must be reported with `# expect: <rule>`; every other line is a negative case. `.after.yml`
//! is the exact `--fix` output (absent when `--fix` changes nothing; regenerate with
//! `CROSGHA_BLESS=1 cargo test --test rules`), and the hand-written `.clean.yml` must not be
//! reported. Online rules run against a local server that replays `tests/fixtures/online/api.json`.

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

const TOKEN: &str = "test-token";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn bin() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crosgha"));
    command.env_remove("GITHUB_ACTIONS");
    command
}

fn fake_github() -> String {
    logged_github().0
}

fn logged_github() -> (String, Arc<Mutex<Vec<String>>>) {
    failing_github(&[])
}

/// An API error the fake server answers instead of the recorded response: the path (`*` for
/// every path), the status line, extra header lines, and GitHub's `message`.
type Failure = (&'static str, &'static str, &'static str, &'static str);

/// Serves recorded GitHub API responses; unknown paths get 404 and a missing token gets 401.
/// The log holds every requested path in arrival order.
fn failing_github(failures: &'static [Failure]) -> (String, Arc<Mutex<Vec<String>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::clone(&log);
    let routes: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&fs::read_to_string(fixtures().join("online/api.json")).unwrap())
            .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            let path = request.split_whitespace().nth(1).unwrap_or("").to_owned();
            requests.lock().unwrap().push(path.clone());
            let mut authorized = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
                authorized |=
                    header.to_ascii_lowercase().trim() == format!("authorization: bearer {TOKEN}");
            }
            let failure = failures
                .iter()
                .find(|(failing, ..)| *failing == "*" || *failing == path);
            let (status, headers, body) = match (failure, routes.get(&path)) {
                _ if !authorized => (
                    "401 Unauthorized",
                    "",
                    r#"{"message":"Bad credentials"}"#.to_owned(),
                ),
                (Some((_, status, headers, message)), _) => (
                    *status,
                    *headers,
                    serde_json::json!({ "message": message }).to_string(),
                ),
                (None, Some(body)) => ("200 OK", "", body.to_string()),
                (None, None) => ("404 Not Found", "", r#"{"message":"Not Found"}"#.to_owned()),
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\n{headers}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (format!("http://{address}"), log)
}

fn scan(path: &Path, fix: bool, api: Option<&str>, config: Option<&Path>) -> Output {
    let mut command = bin();
    command
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .args(["--persona", "auditor", "--fail-on", "none"])
        .args(fix.then_some("--fix"))
        .args(["--format", "json"]);
    match config {
        Some(config) => command.arg("--config").arg(config),
        None => command.arg("--no-config"),
    };
    match api {
        Some(api) => command.args(["--gh-token", TOKEN, "--gh-hostname", api]),
        None => command.arg("--offline"),
    };
    command.arg(path).output().unwrap()
}

fn expected_lines(source: &str, rule: &str) -> BTreeSet<usize> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            line.split_once("# expect: ")
                .is_some_and(|(_, ids)| ids.split(',').any(|id| id.trim() == rule))
        })
        .map(|(index, _)| index + 1)
        .collect()
}

fn reported_lines(
    path: &Path,
    rule: &str,
    api: Option<&str>,
    config: Option<&Path>,
) -> BTreeSet<usize> {
    let output = scan(path, false, api, config);
    assert!(
        output.status.success(),
        "{} exited with {:?}: {}",
        path.display(),
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["rule"] == rule)
        .map(|finding| finding["line"].as_u64().unwrap() as usize)
        .collect()
}

fn check_fixtures(directory: &str, api: Option<&str>) {
    let mut failures = Vec::new();
    let mut checked = 0;
    for entry in fs::read_dir(fixtures().join(directory)).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let Some(stem) = name.strip_suffix(".before.yml") else {
            continue;
        };
        // `<rule>.<variant>.before.yml` covers a second trigger or context for the same rule.
        let rule = stem.split('.').next().unwrap();
        // `<stem>.config.yml` next to a fixture is passed as its crosgha/zizmor config.
        let config = Some(path.with_file_name(format!("{stem}.config.yml")))
            .filter(|config| config.exists());
        let config = config.as_deref();
        checked += 1;
        let source = fs::read_to_string(&path).unwrap();
        let expected = expected_lines(&source, rule);
        if expected.is_empty() {
            failures.push(format!("{name}: no `# expect: {rule}` marker"));
        }
        let reported = reported_lines(&path, rule, api, config);
        if reported != expected {
            failures.push(format!(
                "{name}: expected lines {expected:?}, reported {reported:?}"
            ));
        }

        // `--fix` either removes every finding of the rule or leaves all of them in place.
        let copy = std::env::temp_dir().join(format!("crosgha-{}-{name}", std::process::id()));
        fs::copy(&path, &copy).unwrap();
        scan(&copy, true, api, config);
        let fixed_source = fs::read_to_string(&copy).unwrap();
        let fixed = reported_lines(&copy, rule, api, config).len();
        fs::remove_file(&copy).unwrap();
        if fixed != 0 && fixed != expected.len() {
            failures.push(format!(
                "{name}: --fix left {fixed} of {} findings",
                expected.len()
            ));
        }

        // `.after.yml` records what `--fix` really writes, so it is compared byte for byte and
        // only exists when `--fix` changes the file.
        let after = path.with_file_name(format!("{stem}.after.yml"));
        if std::env::var_os("CROSGHA_BLESS").is_some() {
            if fixed_source == source {
                let _ = fs::remove_file(&after);
            } else {
                fs::write(&after, &fixed_source).unwrap();
            }
        }
        let recorded = fs::read_to_string(&after).unwrap_or_else(|_| source.clone());
        if fixed_source != recorded {
            failures.push(format!(
                "{name}: --fix output differs from {}; review, then rerun with CROSGHA_BLESS=1",
                if after.exists() {
                    format!("{stem}.after.yml")
                } else {
                    format!("{name} (no .after.yml)")
                }
            ));
        }

        // `.clean.yml` applies the recommended remediation by hand and must not be reported.
        let clean = path.with_file_name(format!("{stem}.clean.yml"));
        if !clean.exists() {
            failures.push(format!("{stem}.clean.yml is missing"));
            continue;
        }
        let remaining = reported_lines(&clean, rule, api, config);
        if !remaining.is_empty() {
            failures.push(format!(
                "{stem}.clean.yml still reports lines {remaining:?}"
            ));
        }
    }
    assert!(checked > 0, "no fixtures in {directory}");
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn every_workflow_rule_reports_exactly_the_marked_lines() {
    check_fixtures("rules", None);
}

#[test]
fn every_online_rule_reports_exactly_the_marked_lines() {
    check_fixtures("online", Some(&fake_github()));
}

/// Workflows are inspected concurrently and share one API cache: a batch must report exactly what
/// its files report one by one, in path order, without asking the API twice for the same path.
#[test]
fn a_parallel_batch_matches_single_scans_and_shares_the_api_cache() {
    let batch = std::env::temp_dir().join(format!("crosgha-batch-{}", std::process::id()));
    fs::create_dir_all(&batch).unwrap();
    let mut expected: Vec<(String, String, u64)> = Vec::new();
    for entry in fs::read_dir(fixtures().join("online")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if !name.ends_with(".before.yml") {
            continue;
        }
        let single = scan(&path, false, Some(&fake_github()), None);
        let single: serde_json::Value = serde_json::from_slice(&single.stdout).unwrap();
        for copy in 0..8 {
            let file = format!("{copy}-{name}");
            fs::copy(&path, batch.join(&file)).unwrap();
            for finding in single["findings"].as_array().unwrap() {
                let rule = finding["rule"].as_str().unwrap().to_owned();
                expected.push((file.clone(), rule, finding["line"].as_u64().unwrap()));
            }
        }
    }
    expected.sort();

    let (api, log) = logged_github();
    let output = scan(&batch, false, Some(&api), None);
    fs::remove_dir_all(&batch).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let reported: Vec<(String, String, u64)> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let path = Path::new(finding["path"].as_str().unwrap());
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                finding["rule"].as_str().unwrap().to_owned(),
                finding["line"].as_u64().unwrap(),
            )
        })
        .collect();
    let files: Vec<&String> = reported.iter().map(|(file, ..)| file).collect();
    assert!(files.is_sorted(), "findings out of path order: {files:?}");
    let mut sorted = reported.clone();
    sorted.sort();
    assert_eq!(sorted, expected);

    let log = log.lock().unwrap();
    let mut repeated: Vec<&String> = log
        .iter()
        .filter(|path| log.iter().filter(|other| other == path).count() > 1)
        .collect();
    repeated.dedup();
    assert!(!log.is_empty());
    assert!(
        repeated.is_empty(),
        "requested more than once: {repeated:?}"
    );
}

#[test]
fn online_rules_stay_silent_offline() {
    for entry in fs::read_dir(fixtures().join("online")).unwrap() {
        let path = entry.unwrap().path();
        if !path.to_string_lossy().ends_with(".before.yml") {
            continue;
        }
        let rule = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .split('.')
            .next()
            .unwrap()
            .to_owned();
        assert!(
            reported_lines(&path, &rule, None, None).is_empty(),
            "{rule} ran offline"
        );
    }
}

/// `(file name, rule, line)` of every finding in a JSON report, sorted.
fn finding_keys(report: &serde_json::Value) -> Vec<(String, String, u64)> {
    let mut keys: Vec<(String, String, u64)> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let path = Path::new(finding["path"].as_str().unwrap());
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                finding["rule"].as_str().unwrap().to_owned(),
                finding["line"].as_u64().unwrap(),
            )
        })
        .collect();
    keys.sort();
    keys
}

/// A directory holding online fixtures next to a workflow fixture with an offline fix.
fn mixed_batch(label: &str) -> PathBuf {
    let batch = std::env::temp_dir().join(format!("crosgha-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&batch);
    fs::create_dir_all(&batch).unwrap();
    for fixture in [
        "online/impostor-commit.before.yml",
        "online/known-vulnerable-action.before.yml",
        "rules/persist-credentials.before.yml",
    ] {
        let name = fixture.replace('/', "-");
        fs::copy(fixtures().join(fixture), batch.join(name)).unwrap();
    }
    batch
}

/// A rate-limited token must not cost the offline findings: the online checks are skipped, the
/// first refusal stops every later request, and the incomplete scan exits 2 without writing.
#[test]
fn a_rate_limit_skips_online_checks_and_keeps_the_rest_of_the_report() {
    let batch = mixed_batch("rate-limit");
    let before: Vec<String> = [
        "online-impostor-commit.before.yml",
        "rules-persist-credentials.before.yml",
    ]
    .iter()
    .map(|name| fs::read_to_string(batch.join(name)).unwrap())
    .collect();
    let offline = scan(&batch, false, None, None);
    let (api, log) = failing_github(&[(
        "*",
        "403 Forbidden",
        "x-ratelimit-remaining: 0\r\nx-ratelimit-reset: 4102444800\r\n",
        "API rate limit exceeded for installation ID 1.",
    )]);
    let limited = scan(&batch, true, Some(&api), None);
    let after: Vec<String> = [
        "online-impostor-commit.before.yml",
        "rules-persist-credentials.before.yml",
    ]
    .iter()
    .map(|name| fs::read_to_string(batch.join(name)).unwrap())
    .collect();
    fs::remove_dir_all(&batch).unwrap();

    let stderr = String::from_utf8_lossy(&limited.stderr);
    assert_eq!(limited.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("API rate limit exceeded"), "{stderr}");
    assert!(stderr.contains("the rate limit resets in"), "{stderr}");
    assert!(stderr.contains("incomplete"), "{stderr}");
    assert_eq!(before, after, "an incomplete scan must not write fixes");
    // Each of the 3 workflows pins several actions; only requests already in flight when the
    // first refusal arrived may reach the server, at most one per parallel workflow.
    let requests = log.lock().unwrap().len();
    assert!(
        (1..=3).contains(&requests),
        "{requests} requests despite the rate limit"
    );

    let report: serde_json::Value = serde_json::from_slice(&limited.stdout).unwrap();
    assert_eq!(report["summary"]["files"], 3, "{report:#}");
    assert_eq!(report["summary"]["errors"], 0, "{report:#}");
    // Offline mode adds the archived-action stand-in, which the online rule replaces, and only
    // github.com gets the hosted-runner checks of `retired-runtime`, not the fake host.
    let offline: serde_json::Value = serde_json::from_slice(&offline.stdout).unwrap();
    let mut expected = finding_keys(&offline);
    expected.retain(|(_, rule, _)| rule != "archived-action" && rule != "retired-runtime");
    assert!(!expected.is_empty());
    assert_eq!(finding_keys(&report), expected);
}

/// A server error on one path skips only the check that needed it, on every workflow that asks,
/// and the path is requested once for the whole batch.
#[test]
fn a_failing_api_path_skips_only_the_checks_that_need_it() {
    const FAILING: &str =
        "/repos/actions/checkout/compare/main...2222222222222222222222222222222222222222";
    let batch = mixed_batch("server-error");
    for copy in 0..4 {
        fs::copy(
            fixtures().join("online/impostor-commit.before.yml"),
            batch.join(format!("copy-{copy}.yml")),
        )
        .unwrap();
    }
    let healthy = scan(&batch, false, Some(&fake_github()), None);
    let (api, log) = failing_github(&[(FAILING, "502 Bad Gateway", "", "Server Error")]);
    let failing = scan(&batch, false, Some(&api), None);
    fs::remove_dir_all(&batch).unwrap();

    let stderr = String::from_utf8_lossy(&failing.stderr);
    assert_eq!(failing.status.code(), Some(2), "{stderr}");
    assert_eq!(stderr.matches("HTTP 502").count(), 1, "{stderr}");
    let log = log.lock().unwrap();
    assert_eq!(log.iter().filter(|path| *path == FAILING).count(), 1);

    let healthy: serde_json::Value = serde_json::from_slice(&healthy.stdout).unwrap();
    let failing: serde_json::Value = serde_json::from_slice(&failing.stdout).unwrap();
    let mut expected = finding_keys(&healthy);
    // Line 13 pins the SHA whose comparison fails.
    expected.retain(|(_, rule, line)| !(rule == "impostor-commit" && *line == 13));
    assert_eq!(finding_keys(&failing), expected);
}

/// GITHUB_TOKEN can never read a repository's Actions settings, so a denied read skips only
/// `repo-org-settings`: the report and the SARIF file are still written and the exit status
/// follows the findings. Any other failure of that read makes the scan incomplete.
#[test]
fn unreadable_repository_settings_skip_only_that_rule() {
    const SETTINGS: &str = "/repos/example/project/actions/permissions/workflow";
    const DENIED: &[Failure] = &[(
        SETTINGS,
        "403 Forbidden",
        "",
        "Resource not accessible by integration",
    )];
    const BROKEN: &[Failure] = &[(SETTINGS, "500 Internal Server Error", "", "Server Error")];
    let workflow = fixtures().join("rules/missing-concurrency.before.yml");
    for (failures, repository, code) in [
        (DENIED, "example/project", 0),
        (&[][..], "example/missing", 0),
        (BROKEN, "example/project", 2),
    ] {
        let api = failing_github(failures).0;
        let sarif = std::env::temp_dir().join(
            format!(
                "crosgha-settings-{}-{code}-{repository}.sarif",
                std::process::id()
            )
            .replace('/', "-"),
        );
        let output = bin()
            .env_remove("GH_TOKEN")
            .env_remove("GITHUB_TOKEN")
            .args([
                "--persona",
                "pedantic",
                "--fail-on",
                "none",
                "--format",
                "json",
            ])
            .args(["--repository", repository])
            .args(["--gh-token", TOKEN, "--gh-hostname", &api])
            .arg("--sarif-file")
            .arg(&sarif)
            .arg(&workflow)
            .output()
            .unwrap();
        let written = fs::read_to_string(&sarif);
        let _ = fs::remove_file(&sarif);

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(code), "{repository}: {stderr}");
        assert!(stderr.contains("repo-org-settings was skipped"), "{stderr}");
        if code == 0 {
            assert!(stderr.contains("Administration (read)"), "{stderr}");
        }
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            report["summary"]["findings"].as_u64().unwrap() > 0,
            "{report:#}"
        );
        let sarif: serde_json::Value = serde_json::from_str(&written.unwrap()).unwrap();
        assert_eq!(
            sarif["runs"][0]["results"].as_array().unwrap().len() as u64,
            report["summary"]["findings"].as_u64().unwrap()
        );
    }
}

#[test]
fn repository_settings_are_read_for_the_named_repository() {
    let workflow = fixtures().join("online/archived-action.clean.yml");
    let api = fake_github();
    let output = bin()
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .args([
            "--persona",
            "pedantic",
            "--fail-on",
            "none",
            "--repository",
            "example/project",
        ])
        .args(["--gh-token", TOKEN, "--gh-hostname", &api])
        .arg(&workflow)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let settings: Vec<&str> = stdout
        .lines()
        .filter(|line| line.contains("[repo-org-settings]: "))
        .collect();
    assert_eq!(settings.len(), 2, "{stdout}");
    assert!(
        settings
            .iter()
            .any(|line| line.contains("default_workflow_permissions"))
    );
    assert!(
        settings
            .iter()
            .any(|line| line.contains("can_approve_pull_request_reviews"))
    );

    let rejected = bin()
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .args([
            "--repository",
            "example/project",
            "--gh-token",
            "wrong",
            "--gh-hostname",
            &api,
        ])
        .arg(&workflow)
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("401"));
}

#[test]
fn every_registered_rule_has_a_fixture() {
    let output = bin().arg("--list-rules").output().unwrap();
    assert!(output.status.success());
    let tests: String = ["tests/cli.rs", "tests/rules.rs"]
        .iter()
        .map(|file| {
            fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file)).unwrap()
        })
        .collect();
    let missing: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (rule, scope) = (fields[0], fields[fields.len() - 1]);
            let covered = match scope {
                "workflow" => fixtures().join(format!("rules/{rule}.before.yml")).exists(),
                "online" => fixtures()
                    .join(format!("online/{rule}.before.yml"))
                    .exists(),
                _ => tests.contains(&format!("\"{rule}\"")) || tests.contains(&format!("[{rule}]")),
            };
            (!covered).then(|| format!("{rule} ({scope})"))
        })
        .collect();
    assert!(missing.is_empty(), "rules without fixtures: {missing:?}");
}

#[test]
fn migrate_looks_up_the_latest_crosgha_release() {
    let api = fake_github();
    let path =
        std::env::temp_dir().join(format!("crosgha-migrate-latest-{}.yml", std::process::id()));
    fs::copy(fixtures().join("migrate/zizmor.before.yml"), &path).unwrap();

    let offline = bin()
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .args(["migrate", "--from", "zizmor"])
        .arg(&path)
        .output()
        .unwrap();
    let untouched = fs::read_to_string(&path).unwrap();
    let online = bin()
        .env_remove("GITHUB_TOKEN")
        .env("GH_TOKEN", TOKEN)
        .env("GH_HOST", &api)
        .args(["migrate", "--from", "zizmor"])
        .arg(&path)
        .output()
        .unwrap();
    let migrated = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();

    assert_eq!(offline.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&offline.stderr).contains("--action"));
    assert_eq!(
        untouched,
        fs::read_to_string(fixtures().join("migrate/zizmor.before.yml")).unwrap()
    );
    assert!(
        online.status.success(),
        "{}",
        String::from_utf8_lossy(&online.stderr)
    );
    assert_eq!(
        migrated,
        fs::read_to_string(fixtures().join("migrate/zizmor.after.yml")).unwrap()
    );
}
