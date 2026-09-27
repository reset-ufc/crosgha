//! Data-driven checks for every rule. Each `<rule>.before.yml` fixture marks the lines that
//! must be reported with `# expect: <rule>`; every other line is a negative case. The matching
//! `.after.yml` is the corrected workflow and must be clean. Online rules run against a local
//! server that replays `tests/fixtures/online/api.json`.

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TOKEN: &str = "test-token";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_crosgha"))
}

/// Serves recorded GitHub API responses; unknown paths get 404 and a missing token gets 401.
fn fake_github() -> String {
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
            let (status, body) = match routes.get(&path) {
                _ if !authorized => (
                    "401 Unauthorized",
                    r#"{"message":"Bad credentials"}"#.to_owned(),
                ),
                Some(body) => ("200 OK", body.to_string()),
                None => ("404 Not Found", r#"{"message":"Not Found"}"#.to_owned()),
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    format!("http://{address}")
}

fn scan(path: &Path, fix: bool, api: Option<&str>, config: Option<&Path>) -> Output {
    let mut command = bin();
    command
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .args(["--persona", "auditor", "--fail-on", "none"])
        .args(fix.then_some("--fix"));
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    let anchor = format!(" {}:", path.display());
    stdout
        .lines()
        .filter(|line| line.split_whitespace().next() == Some(rule))
        .map(|line| {
            let after = &line[line.find(&anchor).unwrap() + anchor.len()..];
            after[..after.find(':').unwrap()].parse().unwrap()
        })
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
        let fixed = reported_lines(&copy, rule, api, config).len();
        fs::remove_file(&copy).unwrap();
        if fixed != 0 && fixed != expected.len() {
            failures.push(format!(
                "{name}: --fix left {fixed} of {} findings",
                expected.len()
            ));
        }

        let after = path.with_file_name(format!("{stem}.after.yml"));
        if !after.exists() {
            failures.push(format!("{stem}.after.yml is missing"));
            continue;
        }
        let remaining = reported_lines(&after, rule, api, config);
        if !remaining.is_empty() {
            failures.push(format!(
                "{stem}.after.yml still reports lines {remaining:?}"
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

#[test]
fn repository_settings_are_read_for_the_named_repository() {
    let workflow = fixtures().join("online/archived-action.after.yml");
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
        .filter(|line| line.starts_with("repo-org-settings ["))
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
                _ => tests.contains(&format!("\"{rule} [\"")),
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
