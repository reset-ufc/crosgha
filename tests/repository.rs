#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! Rules that read the repository around the workflows: Dependabot, security tooling, and local actions.

mod common;

use common::{bin, diagnostics, fixture};
use std::path::PathBuf;

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
