#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! Rules that read the repository around the workflows: Dependabot, security tooling, and local
//! actions, in a local directory and in a remote repository read through the fake GitHub API.

mod common;

use common::{
    Api, COMMIT, Failure, TOKEN, TempDir, bin, copy_tree, diagnostics, fixture, github_api,
    hex_decode, percent_decode,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn missing_release_cooldown_and_missing_security_tooling_check_the_repository() {
    let scan = |repo: &str| {
        let output = bin()
            .args(["--persona", "auditor", "--offline", "--fail-on", "none"])
            .arg(fixture(&format!("repos/{repo}/.github/workflows")))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let insecure = scan("insecure");
    let cooldown = diagnostics(&insecure, "missing-release-cooldown");
    // Line 13 sets only `semver-major-days`, so other updates keep Dependabot's 3-day default.
    assert_eq!(cooldown.len(), 2, "{insecure}");
    assert!(cooldown[0].1.contains("dependabot.yml:9:"), "{insecure}");
    assert!(cooldown[1].1.contains("dependabot.yml:13:"), "{insecure}");
    let tooling = diagnostics(&insecure, "missing-security-tooling");
    assert_eq!(tooling.len(), 3, "{insecure}");
    for tool in ["CodeQL", "dependency review", "OpenSSF Scorecard"] {
        assert!(
            tooling.iter().any(|(header, _)| header.contains(tool)),
            "{tool}: {insecure}"
        );
    }

    // Dependabot waits 3 days even without `cooldown`, which meets a 3-day minimum.
    let output = bin()
        .args([
            "--persona",
            "auditor",
            "--offline",
            "--fail-on",
            "none",
            "--config",
        ])
        .arg(fixture("cooldown-3-days.yml"))
        .arg(fixture("repos/insecure/.github/workflows"))
        .output()
        .unwrap();
    let relaxed = String::from_utf8(output.stdout).unwrap();
    assert!(!relaxed.contains("[missing-release-cooldown]"), "{relaxed}");

    let hardened = scan("hardened");
    assert!(
        !hardened.contains("[missing-release-cooldown]"),
        "{hardened}"
    );
    assert!(
        !hardened.contains("[missing-security-tooling]"),
        "{hardened}"
    );
}

#[test]
fn retired_runtime_flags_local_actions_of_the_repository() {
    let output = bin()
        .args(["--offline", "--fail-on", "none"])
        .arg(fixture("repos/runtime/.github/workflows"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut retired: Vec<String> = diagnostics(&stdout, "retired-runtime")
        .into_iter()
        .map(|(_, at)| {
            let location = at.split("repos/runtime/").nth(1).unwrap();
            location.rsplit_once(':').unwrap().0.to_owned()
        })
        .collect();
    retired.sort();
    // node24 and composite actions pass; a reused action is reported once, a missing one not at all.
    assert_eq!(
        retired,
        [
            ".github/actions/old/action.yml:4",
            "action.yml:4",
            "tools/legacy/action.yaml:4",
        ],
        "{stdout}"
    );
}

#[test]
fn github_com_only_rules_are_skipped_for_another_host() {
    let scan = |host: &str, path: PathBuf| -> Vec<String> {
        let output = bin()
            .args([
                "--offline",
                "--fail-on",
                "none",
                "-f",
                "json",
                "--gh-hostname",
                host,
            ])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        json["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| finding["rule"].as_str().unwrap().to_owned())
            .filter(|rule| rule == "removed-feature" || rule == "retired-runtime")
            .collect()
    };
    let hosted = fixture("rules/removed-feature.before.yml");
    let java = fixture("rules/removed-feature.setup-java.before.yml");
    let runtime = fixture("repos/runtime/.github/workflows");

    let on_github = scan("github.com", hosted.clone());
    assert!(on_github.len() > 5);
    // Host names are case-insensitive, and a trailing slash names the same API.
    for spelling in ["GitHub.COM", "github.com/", "https://api.github.com/"] {
        assert_eq!(scan(spelling, hosted.clone()), on_github, "{spelling}");
    }
    assert_eq!(scan("ghe.example.com", hosted), Vec::<String>::new());
    // The setup-java distribution removal is a fact about the action, not about the host.
    assert_eq!(scan("ghe.example.com", java), ["removed-feature"; 2]);
    assert_eq!(scan("github.com", runtime.clone()).len(), 3);
    assert_eq!(scan("ghe.example.com", runtime), Vec::<String>::new());
}

#[test]
fn dependabot_config_input_is_audited_not_parsed_as_a_workflow() {
    let dependabot = fixture("repos/cooldown/.github/dependabot.yml");
    let scan = |paths: &[PathBuf]| {
        let output = bin()
            .args(["--persona", "auditor", "--offline", "--fail-on", "none"])
            .args(paths)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    // Config is discovered next to the file: `.github/crosgha.yml` raises the minimum to 12 days.
    let alone = scan(std::slice::from_ref(&dependabot));
    let cooldown = diagnostics(&alone, "missing-release-cooldown");
    assert_eq!(cooldown.len(), 2, "{alone}");
    assert!(cooldown[0].1.contains("dependabot.yml:3:"), "{alone}");
    assert!(cooldown[1].1.contains("dependabot.yml:9:"), "{alone}");
    // A lone Dependabot file says nothing about the repository's workflows.
    assert!(!alone.contains("[missing-security-tooling]"), "{alone}");

    let both = scan(&[fixture("repos/cooldown/.github/workflows"), dependabot]);
    assert_eq!(
        diagnostics(&both, "missing-release-cooldown").len(),
        2,
        "{both}"
    );
}

/// What one scan says about one repository, with every path relative to the repository root.
#[derive(Debug, PartialEq)]
struct Reported {
    status: Option<i32>,
    files: u64,
    errors: usize,
    /// `(path, line, column, rule, message)` of each finding in the JSON report.
    findings: Vec<(String, u64, u64, String, String)>,
    /// `(rule, path, line)` of each SARIF result that has a location; a repository-wide finding
    /// is placed on a file.
    sarif: Vec<(String, String, u64)>,
}

/// Runs `command` on one repository whose report paths start with `prefix` (in SARIF, `marker`
/// ends the part of each URI that is dropped).
fn reported(mut command: Command, prefix: &str, marker: &str) -> Reported {
    let temp = TempDir::new("equivalence");
    let sarif_path = temp.join("report.sarif");
    let output = command
        .args(["--format", "json", "--sarif-file"])
        .arg(&sarif_path)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{error}: {stderr}"));
    let relative = |path: &str| {
        let relative = path
            .strip_prefix(prefix)
            .unwrap_or_else(|| panic!("{path} is not in {prefix}"));
        relative.trim_start_matches('/').to_owned()
    };
    let mut findings: Vec<_> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            (
                relative(finding["path"].as_str().unwrap()),
                finding["line"].as_u64().unwrap(),
                finding["column"].as_u64().unwrap(),
                finding["rule"].as_str().unwrap().to_owned(),
                finding["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    findings.sort();
    let sarif: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sarif_path).unwrap()).unwrap();
    let mut results: Vec<_> = sarif["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|result| {
            let location = &result["locations"][0]["physicalLocation"];
            let uri = percent_decode(location["artifactLocation"]["uri"].as_str()?);
            let (_, path) = uri
                .split_once(marker)
                .unwrap_or_else(|| panic!("{uri} is not in {marker}"));
            Some((
                result["ruleId"].as_str().unwrap().to_owned(),
                path.to_owned(),
                location["region"]["startLine"].as_u64().unwrap(),
            ))
        })
        .collect();
    results.sort();
    Reported {
        status: output.status.code(),
        files: report["summary"]["files"].as_u64().unwrap(),
        errors: report["errors"].as_array().unwrap().len(),
        findings,
        sarif: results,
    }
}

/// The same repository scanned from a local directory and as `fixture/<name>` through the fake
/// API, which serves that directory at `COMMIT`; both runs take the same `args`.
fn local_and_remote(repository: &Path, api: &str, args: &[&str]) -> (Reported, Reported) {
    let name = repository.file_name().unwrap().to_str().unwrap();
    let mut local = bin();
    local
        .args(["--fail-on", "none", "--gh-hostname", api])
        .args(args)
        .arg(repository);
    let local = reported(
        local,
        &repository.display().to_string(),
        &format!("/{name}/"),
    );
    let mut remote = bin();
    let input = format!("fixture/{name}");
    remote
        .args([
            "--fail-on",
            "none",
            "--gh-hostname",
            api,
            "--gh-token",
            TOKEN,
        ])
        .args(args)
        .arg(&input);
    let prefix = format!("{input}@{COMMIT}:");
    let remote = reported(remote, &prefix, &prefix);
    (local, remote)
}

/// Every fixture repository (plus `composite` with a symbolic link to one of its actions) gives the
/// same report scanned locally and remotely at the same commit: workflows, the root action and
/// local actions it reaches, the repository rules, the config it discovers, and where SARIF places
/// repository-wide findings. `--no-online-audits` still reads the repository, and nothing else.
#[test]
fn a_remote_repository_reports_what_its_local_copy_reports() {
    let (api, requests) = github_api(Api::default());
    let mut repositories: Vec<PathBuf> = fs::read_dir(fixture("repos"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    repositories.sort();
    let args = ["--persona", "auditor", "--no-online-audits"];
    let mut checked = Vec::new();
    for repository in &repositories {
        let (local, remote) = local_and_remote(repository, &api, &args);
        assert_eq!(remote, local, "{}", repository.display());
        checked.extend(local.findings.into_iter().map(|finding| finding.3));
    }
    for rule in [
        "missing-release-cooldown",
        "missing-security-tooling",
        "deprecated-command",
        "script-injection",
    ] {
        assert!(
            checked.iter().any(|checked| checked == rule),
            "no fixture repository reported {rule}"
        );
    }
    let read: Vec<String> = requests.lock().unwrap().clone();
    let unexpected: Vec<&String> = read
        .iter()
        .filter(|path| {
            let endpoint = path
                .strip_prefix("/repos/fixture/")
                .and_then(|path| path.split_once('/'))
                .map(|(_, endpoint)| endpoint);
            !endpoint.is_some_and(|endpoint| {
                ["commits/", "git/trees/", "contents/"]
                    .iter()
                    .any(|allowed| endpoint.starts_with(allowed))
            })
        })
        .collect();
    assert!(
        unexpected.is_empty(),
        "--no-online-audits made API requests: {unexpected:?}"
    );

    // A symbolic link is not followed remotely (the API lists it without its target), and locally
    // it is the action it points to, which the workflow also calls: the reports still agree.
    let temp = TempDir::new("linked-repository");
    let copy = temp.join("composite");
    copy_tree(&fixture("repos/composite"), &copy);
    std::os::unix::fs::symlink("setup", copy.join(".github/actions/alias")).unwrap();
    let (linked_api, _) = github_api(Api {
        repositories: temp.to_path_buf(),
        ..Api::default()
    });
    let (local, remote) = local_and_remote(&copy, &linked_api, &args);
    assert_eq!(remote, local);
    assert_eq!(remote.files, 5, "{remote:#?}");
}

/// `repo-org-settings` checks the remote input's own repository, even when `GITHUB_REPOSITORY`
/// names another one, and reports what a local copy reports with `--repository` naming it.
#[test]
fn a_remote_repository_checks_its_own_actions_settings() {
    let (api, requests) = github_api(Api::default());
    let repository = fixture("repos/repo-org-settings");
    let mut local = bin();
    local
        .args(["--persona", "pedantic", "--fail-on", "none"])
        .args(["--gh-hostname", &api, "--gh-token", TOKEN])
        .args(["--repository", "fixture/repo-org-settings"])
        .arg(&repository);
    let local = reported(
        local,
        &repository.display().to_string(),
        "/repo-org-settings/",
    );
    let mut remote = bin();
    remote
        .env("GITHUB_REPOSITORY", "example/project")
        .args(["--persona", "pedantic", "--fail-on", "none"])
        .args(["--gh-hostname", &api, "--gh-token", TOKEN])
        .arg("fixture/repo-org-settings");
    let prefix = format!("fixture/repo-org-settings@{COMMIT}:");
    let remote = reported(remote, &prefix, &prefix);
    assert_eq!(remote, local);
    let settings: Vec<_> = remote
        .sarif
        .iter()
        .filter(|(rule, ..)| rule == "repo-org-settings")
        .collect();
    assert_eq!(
        settings,
        [&(
            "repo-org-settings".to_owned(),
            ".github/workflows/ci.yml".to_owned(),
            1
        )],
        "{remote:#?}"
    );
    assert!(
        !requests
            .lock()
            .unwrap()
            .iter()
            .any(|path| path.contains("example/project")),
        "GITHUB_REPOSITORY was read for a remote input"
    );
}

/// GitHub truncates a recursive tree over 100,000 entries or 7 MB. The scan then lists, one
/// request each, only the directories its lookups need, and reports what the full tree reports.
#[test]
fn a_truncated_tree_is_read_one_directory_at_a_time() {
    let args = ["--persona", "auditor", "--no-online-audits"];
    let repository = fixture("repos/composite");
    let (api, _) = github_api(Api::default());
    let (local, complete) = local_and_remote(&repository, &api, &args);
    assert_eq!(complete, local);
    let (api, requests) = github_api(Api {
        truncated: true,
        ..Api::default()
    });
    let (_, truncated) = local_and_remote(&repository, &api, &args);
    assert_eq!(truncated, complete);

    let trees = "/repos/fixture/composite/git/trees/";
    let mut listed: Vec<String> = requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|path| path.strip_prefix(trees))
        .filter(|tree| !tree.ends_with("?recursive=1"))
        .map(|tree| {
            let decoded = hex_decode(tree).unwrap();
            decoded.strip_prefix("tree:").unwrap().to_owned()
        })
        .collect();
    listed.sort();
    // The root and `.github` hold the configs and Dependabot files; the rest lead to the
    // workflows and to each called action, and `tools/unused`, which nothing calls, is not read.
    assert_eq!(
        listed,
        [
            "",
            ".github",
            ".github/actions",
            ".github/actions/cache",
            ".github/actions/node",
            ".github/actions/setup",
            ".github/workflows",
            "docker",
        ]
    );
}

/// A file the tree lists but the API refuses, or no longer has, is an error of that file: the rest
/// of the repository is still reported and the run exits 2. A refused Dependabot config stops the
/// run, as an unreadable config does.
#[test]
fn a_remote_file_that_cannot_be_read_fails_only_that_file() {
    let contents = |repository: &str, path: &str| -> &'static str {
        format!("/repos/fixture/{repository}/contents/{path}?ref={COMMIT}").leak()
    };
    let node = contents("composite", ".github/actions/node/action.yml");
    let refused: &'static [Failure] =
        vec![(node, "403 Forbidden", "", "Resource not accessible")].leak();
    let gone: &'static [Failure] = vec![(node, "404 Not Found", "", "Not Found")].leak();
    for (failures, reason) in [(refused, "HTTP 403"), (gone, "disappeared")] {
        let (api, _) = github_api(Api {
            failures,
            ..Api::default()
        });
        let output = bin()
            .args(["--no-online-audits", "--no-config", "-f", "json"])
            .args(["--gh-hostname", &api, "--gh-token", TOKEN])
            .arg("fixture/composite")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{stderr}");
        assert!(
            stderr.contains(".github/actions/node/action.yml") && stderr.contains(reason),
            "{stderr}"
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["errors"].as_array().unwrap().len(), 1, "{report:#}");
        assert!(
            report["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| {
                    finding["rule"] == "deprecated-command"
                        && finding["path"]
                            .as_str()
                            .unwrap()
                            .ends_with(".github/actions/setup/action.yml")
                }),
            "{report:#}"
        );
    }

    let dependabot = contents("insecure", ".github/dependabot.yml");
    let limited: &'static [Failure] = vec![(
        dependabot,
        "403 Forbidden",
        "x-ratelimit-remaining: 0\r\nx-ratelimit-reset: 9999999999\r\n",
        "API rate limit exceeded",
    )]
    .leak();
    let (api, _) = github_api(Api {
        failures: limited,
        ..Api::default()
    });
    let output = bin()
        .args(["--persona", "pedantic", "--no-online-audits", "--no-config"])
        .args(["--gh-hostname", &api, "--gh-token", TOKEN])
        .arg("fixture/insecure")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("rate limit"), "{stderr}");
}
