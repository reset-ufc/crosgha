use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crosgha"));
    command
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_ACTIONS");
    command
}

/// `(header, path:line:col)` of each `rule` diagnostic in a text report.
fn diagnostics<'a>(report: &'a str, rule: &str) -> Vec<(&'a str, &'a str)> {
    let header = format!("[{rule}]: ");
    let lines: Vec<&str> = report.lines().collect();
    lines
        .windows(2)
        .filter(|pair| pair[0].contains(&header))
        .map(|pair| (pair[0], pair[1].trim_start().trim_start_matches("--> ")))
        .collect()
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
            .contains("Fixed 3 findings")
    );

    let checked = bin()
        .arg(fixture("checkout-credentials.after.yml"))
        .output()
        .unwrap();
    assert!(checked.status.success());
}

/// GitHub runs workflows saved with a UTF-8 BOM (common from Windows editors); the benchmark
/// corpus has them with a key on the first line, which the YAML parser used to reject.
#[test]
fn byte_order_mark_is_scanned_like_plain_utf8_and_kept_on_fix() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("crosgha-bom-{}-{unique}.yml", std::process::id()));
    let plain = fs::read_to_string(fixture("checkout-credentials.before.yml")).unwrap();
    fs::write(&path, format!("\u{feff}{plain}")).unwrap();
    let findings = |path: &std::path::Path| {
        let output = bin()
            .args(["--persona", "auditor", "-f", "json", "--fail-on", "none"])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| (finding["rule"].to_string(), finding["line"].to_string()))
            .collect::<Vec<_>>()
    };

    let with_bom = findings(&path);
    let fix = bin().arg("--fix").arg(&path).output().unwrap();
    let fixed = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();

    assert!(!with_bom.is_empty());
    assert_eq!(
        with_bom,
        findings(&fixture("checkout-credentials.before.yml"))
    );
    assert!(
        fix.status.success(),
        "{}",
        String::from_utf8_lossy(&fix.stderr)
    );
    let after = fs::read_to_string(fixture("checkout-credentials.after.yml")).unwrap();
    assert_eq!(fixed, format!("\u{feff}{after}"));
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

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(before, after);
}

#[test]
fn invalid_workflow_does_not_hide_findings_in_the_others() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("crosgha-mixed-{}-{unique}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let valid = dir.join("ci.yml");
    fs::copy(fixture("checkout-credentials.before.yml"), &valid).unwrap();
    fs::copy(fixture("malformed.yml"), dir.join("broken.yml")).unwrap();
    let before = fs::read(&valid).unwrap();

    let report = bin().arg(&dir).output().unwrap();
    let json = bin().args(["-f", "json"]).arg(&dir).output().unwrap();
    let sarif = bin().args(["-f", "sarif"]).arg(&dir).output().unwrap();
    let fix = bin().arg("--fix").arg(&dir).output().unwrap();
    let after = fs::read(&valid).unwrap();
    fs::remove_dir_all(&dir).unwrap();

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
        stdout.matches("[persist-credentials]").count(),
        3,
        "{stdout}"
    );
    assert!(!stdout.contains("Fixed"), "{stdout}");
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
fn fixes_checkout_while_reporting_a_detection_only_finding() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("crosgha-mixed-{}-{unique}.yml", std::process::id()));
    fs::copy(fixture("mixed-fixes.before.yml"), &path).unwrap();

    let regular = bin().arg(&path).output().unwrap();
    let output = bin()
        .args(["--persona", "pedantic", "--fix"])
        .arg(&path)
        .output()
        .unwrap();
    let actual = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();

    let regular = String::from_utf8(regular.stdout).unwrap();
    assert!(regular.contains("[persist-credentials]"));
    assert!(!regular.contains("[unpinned-action]"));
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        actual,
        fs::read_to_string(fixture("mixed-fixes.after.yml")).unwrap()
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Fixed 1 finding in"));
    assert_eq!(stdout.matches("[unpinned-action]").count(), 1);
    assert!(!stdout.contains("[persist-credentials]"));
}

#[cfg(unix)]
#[test]
fn scans_symlinked_workflows_in_directories_but_refuses_to_fix_them() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("crosgha-links-{}-{unique}", std::process::id()));
    let scanned = root.join("workflows");
    fs::create_dir_all(&scanned).unwrap();
    let target = root.join("shared.yml");
    fs::copy(fixture("checkout-credentials.before.yml"), &target).unwrap();
    fs::copy(fixture("workflows/ci.yml"), scanned.join("ci.yml")).unwrap();
    std::os::unix::fs::symlink(&target, scanned.join("linked.yml")).unwrap();
    let before = fs::read(&target).unwrap();

    let scan = bin().arg(&scanned).output().unwrap();
    let fix = bin().arg("--fix").arg(&scanned).output().unwrap();
    let after = fs::read(&target).unwrap();
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(scan.status.code(), Some(1));
    let stdout = String::from_utf8(scan.stdout).unwrap();
    assert!(stdout.contains("Checked 2 workflow files: 3 findings"));
    assert!(stdout.contains("linked.yml"));
    assert_eq!(fix.status.code(), Some(2));
    assert!(
        String::from_utf8(fix.stderr)
            .unwrap()
            .contains("refusing to replace a symbolic link")
    );
    assert_eq!(before, after);
}

#[test]
fn inline_ignores_accept_crosgha_and_zizmor_rule_ids() {
    let output = bin().arg(fixture("ignores.yml")).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("[persist-credentials]"), "{stdout}");
    let injections = diagnostics(&stdout, "script-injection");
    assert_eq!(injections.len(), 2, "{stdout}");
    assert!(injections[0].1.contains("ignores.yml:12:"));
    assert!(injections[1].1.contains("ignores.yml:13:"));
}

#[test]
fn repository_rules_check_dependabot_cooldown_and_security_tooling() {
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
fn repository_rules_flag_retired_node_runtimes() {
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

    assert!(scan("github.com", hosted.clone()).len() > 5);
    assert_eq!(scan("ghe.example.com", hosted), Vec::<String>::new());
    // The setup-java distribution removal is a fact about the action, not about the host.
    assert_eq!(scan("ghe.example.com", java), ["removed-feature"; 2]);
    assert_eq!(scan("github.com", runtime.clone()).len(), 3);
    assert_eq!(scan("ghe.example.com", runtime), Vec::<String>::new());
}

#[test]
fn zizmor_config_is_discovered_and_honored() {
    let workflows = fixture("repos/configured/.github/workflows");
    let scan = |extra: &[&str]| {
        let output = bin()
            .args(["--persona", "pedantic", "--offline", "--fail-on", "none"])
            .args(extra)
            .arg(&workflows)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let lines_of = |stdout: &str, rule: &str| -> Vec<String> {
        diagnostics(stdout, rule)
            .into_iter()
            .map(|(_, at)| {
                at.split(".yml:")
                    .nth(1)
                    .unwrap()
                    .split(':')
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    };

    let configured = scan(&[]);
    assert_eq!(
        lines_of(&configured, "script-injection"),
        ["15"],
        "{configured}"
    );
    assert!(
        lines_of(&configured, "unpinned-action").is_empty(),
        "{configured}"
    );
    assert!(
        configured.contains("error[persist-credentials]"),
        "{configured}"
    );

    let unignored = scan(&["--no-ignores"]);
    assert_eq!(
        lines_of(&unignored, "script-injection"),
        ["14", "15"],
        "{unignored}"
    );
    assert!(
        lines_of(&unignored, "unpinned-action").is_empty(),
        "{unignored}"
    );

    let unconfigured = scan(&["--no-config"]);
    assert_eq!(lines_of(&unconfigured, "script-injection"), ["14", "15"]);
    // `--no-config` also wins over a config named in the environment.
    for variable in ["CROSGHA_CONFIG", "ZIZMOR_CONFIG"] {
        let output = bin()
            .args(["--persona", "pedantic", "--offline", "--fail-on", "none"])
            .arg("--no-config")
            .env(variable, fixture("repos/configured/.github/zizmor.yml"))
            .arg(&workflows)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{variable}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), unconfigured);
    }
    assert_eq!(lines_of(&unconfigured, "unpinned-action"), ["11"]);
    assert!(unconfigured.contains("warning[persist-credentials]"));

    let severe = scan(&["--no-config", "--min-severity", "high"]);
    assert!(
        !severe.contains("warning[") && !severe.contains("note["),
        "{severe}"
    );
    assert_eq!(lines_of(&severe, "script-injection"), ["14", "15"]);
}

#[test]
fn migrate_replaces_other_scanners_with_the_crosgha_action() {
    let action = format!("meiazero/crosgha@{}", "c".repeat(40));
    for tool in ["zizmor", "actionlint"] {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("crosgha-migrate-{tool}-{unique}.yml"));
        fs::copy(fixture(&format!("migrate/{tool}.before.yml")), &path).unwrap();

        let output = bin()
            .args([
                "migrate",
                "--from",
                tool,
                "--action",
                &action,
                "--action-version",
                "v0.1.0",
            ])
            .arg(&path)
            .output()
            .unwrap();
        let migrated = fs::read_to_string(&path).unwrap();
        let again = bin()
            .args([
                "migrate",
                "--from",
                tool,
                "--action",
                &action,
                "--action-version",
                "v0.1.0",
            ])
            .arg(&path)
            .output()
            .unwrap();
        let unchanged = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).unwrap();

        assert!(
            output.status.success(),
            "{tool}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            migrated,
            fs::read_to_string(fixture(&format!("migrate/{tool}.after.yml"))).unwrap(),
            "{tool}"
        );
        assert!(again.status.success());
        assert_eq!(
            migrated, unchanged,
            "{tool}: a second migration must be a no-op"
        );
    }
}

#[test]
fn report_names_the_equivalent_rule_in_other_tools() {
    let output = bin()
        .args(["--persona", "pedantic", "--offline", "--no-config"])
        .arg(fixture("repos/configured/.github/workflows/ci.yml"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let diagnostic = |rule: &str| {
        let start = stdout
            .find(&format!("[{rule}]: "))
            .unwrap_or_else(|| panic!("no {rule}: {stdout}"));
        stdout[start..].split("\n\n").next().unwrap().to_owned()
    };
    assert!(
        diagnostic("script-injection")
            .contains("= note: same as zizmor: template-injection; actionlint: expression")
    );
    assert!(
        diagnostic("unpinned-action")
            .contains("= note: same as zizmor: unpinned-uses, unpinned-images")
    );
}

#[test]
fn cooldown_days_follow_the_configured_minimum() {
    let scan = |extra: &[&str]| {
        let output = bin()
            .args(["--persona", "pedantic", "--offline", "--fail-on", "none"])
            .args(extra)
            .arg(fixture("repos/cooldown/.github/workflows"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let lines: Vec<String> = diagnostics(&stdout, "missing-release-cooldown")
            .into_iter()
            .map(|(_, at)| {
                at.split("dependabot.yml:")
                    .nth(1)
                    .unwrap()
                    .split(':')
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        lines
    };
    assert_eq!(scan(&[]), ["3", "9"]);
    assert_eq!(scan(&["--no-config"]), ["3"]);
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

#[test]
fn config_blocks_without_a_crosgha_reader_are_reported() {
    let output = bin()
        .args(["--offline", "--config"])
        .arg(fixture("unsupported-config.yml"))
        .arg(fixture("workflows"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("`rules.secrets-outside-env.config` has no crosgha equivalent"),
        "{stderr}"
    );
    assert!(!stderr.contains("unpinned-uses"), "{stderr}");
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn migrate_converts_configs_and_shell_steps() {
    let action = format!("meiazero/crosgha@{}", "c".repeat(40));
    for (tool, workflow, config) in [
        ("zizmor", "ci.yml", "zizmor.yml"),
        ("actionlint", "lint.yml", "actionlint.yaml"),
    ] {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("crosgha-migrate-repo-{tool}-{unique}"));
        copy_tree(&fixture(&format!("migrate/{tool}-repo")), &repo);

        let output = bin()
            .args([
                "migrate",
                "--from",
                tool,
                "--action",
                &action,
                "--action-version",
                "v0.1.0",
            ])
            .arg(repo.join(".github/workflows"))
            .output()
            .unwrap();
        let migrated = fs::read_to_string(repo.join(".github/workflows").join(workflow)).unwrap();
        let crosgha = fs::read_to_string(repo.join(".github/crosgha.yml")).unwrap_or_default();
        let original = fs::read_to_string(repo.join(".github").join(config)).unwrap();
        let scan = bin()
            .args(["--offline", "--fail-on", "none"])
            .arg(repo.join(".github/workflows"))
            .output()
            .unwrap();
        fs::remove_dir_all(&repo).unwrap();

        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{tool}: {stderr}");
        let expected = |name: &str| {
            fs::read_to_string(fixture(&format!("migrate/expected/{tool}-repo.{name}"))).unwrap()
        };
        assert_eq!(migrated, expected(workflow), "{tool}");
        assert_eq!(crosgha, expected("crosgha.yml"), "{tool}");
        assert_eq!(
            original,
            fs::read_to_string(fixture(&format!("migrate/{tool}-repo/.github/{config}"))).unwrap(),
            "{tool}: the original config stays for as long as {tool} runs"
        );
        assert!(
            scan.status.success(),
            "{tool}: {}",
            String::from_utf8_lossy(&scan.stderr)
        );
        match tool {
            "zizmor" => {
                assert!(
                    stderr.contains("`use-trusted-publishing` has no crosgha rule"),
                    "{stderr}"
                );
                assert!(stderr.contains("ci.yml:21"), "{stderr}");
                // Steps that install zizmor next to other tools or commands stay, with a note.
                for line in ["ci.yml:32", "ci.yml:35"] {
                    assert!(
                        stderr
                            .lines()
                            .any(|note| note.contains(line)
                                && note.contains("next to other commands")),
                        "{line}: {stderr}"
                    );
                }
                assert!(
                    stderr.lines().any(|note| note.contains("ci.yml:37")
                        && note.contains("`--format json` has no crosgha Action input")),
                    "{stderr}"
                );
            }
            _ => {
                assert!(stderr.contains("`config-variables`"), "{stderr}");
                assert!(stderr.contains("`paths`"), "{stderr}");
            }
        }
    }
}

#[test]
fn every_format_reports_the_findings_of_the_text_report() {
    let repo = fixture("repos/configured");
    let scan = |format: &str| {
        let output = bin()
            .current_dir(&repo)
            .args([
                "--persona",
                "pedantic",
                "--offline",
                "--no-config",
                "--format",
                format,
            ])
            .arg(".github/workflows")
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let text = scan("text");
    let count = text
        .lines()
        .filter(|line| {
            ["error[", "warning[", "note["]
                .iter()
                .any(|level| line.starts_with(level))
        })
        .count();
    assert!(count >= 3, "{text}");

    let json: serde_json::Value = serde_json::from_str(&scan("json")).unwrap();
    let findings = json["findings"].as_array().unwrap();
    assert_eq!(findings.len(), count);
    assert_eq!(json["summary"]["findings"], count);
    let injection = findings
        .iter()
        .find(|finding| finding["rule"] == "script-injection")
        .unwrap();
    assert_eq!(injection["path"], ".github/workflows/ci.yml");
    assert_eq!(injection["line"], 14);
    assert_eq!(injection["severity"], "error");
    assert_eq!(injection["equivalents"][0]["tool"], "zizmor");
    assert_eq!(injection["equivalents"][0]["rule"], "template-injection");

    let agent = scan("agent");
    assert_eq!(agent.lines().count(), count, "{agent}");
    for line in agent.lines() {
        assert!(line.starts_with(".github/workflows/"), "{line}");
        assert!(line.contains(" help: "), "{line}");
    }
    let injection = agent
        .lines()
        .find(|line| line.contains(" script-injection: "))
        .unwrap();
    assert!(
        injection.starts_with(".github/workflows/ci.yml:14:")
            && injection.contains(": error script-injection: ")
            && injection.contains("template-injection"),
        "{injection}"
    );

    let sarif: serde_json::Value = serde_json::from_str(&scan("sarif")).unwrap();
    assert_eq!(sarif["version"], "2.1.0");
    assert!(sarif["$schema"].as_str().unwrap().contains("sarif-2.1.0"));
    let run = &sarif["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "crosgha");
    assert_eq!(run["invocations"][0]["executionSuccessful"], true, "{run}");
    let rules = run["tool"]["driver"]["rules"].as_array().unwrap();
    for rule in rules {
        for field in ["shortDescription", "fullDescription", "help"] {
            assert!(!rule[field]["text"].as_str().unwrap().is_empty(), "{rule}");
        }
    }
    let results = run["results"].as_array().unwrap();
    assert_eq!(results.len(), count);
    for result in results {
        let rule = &rules[result["ruleIndex"].as_u64().unwrap() as usize];
        assert_eq!(rule["id"], result["ruleId"]);
        assert!(["note", "warning", "error"].contains(&result["level"].as_str().unwrap()));
        let location = &result["locations"][0]["physicalLocation"];
        assert!(
            !location["artifactLocation"]["uri"]
                .as_str()
                .unwrap()
                .starts_with('/')
        );
        let region = &location["region"];
        assert!(region["startLine"].as_u64().unwrap() >= 1);
        assert!(region["startColumn"].as_u64().unwrap() >= 1);
        assert!(region["endLine"].as_u64() >= region["startLine"].as_u64());
        assert!(region["endColumn"].as_u64().unwrap() >= 1);
    }
    let injection = results
        .iter()
        .find(|result| result["ruleId"] == "script-injection")
        .unwrap();
    assert_eq!(
        injection["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        ".github/workflows/ci.yml"
    );
    assert_eq!(
        injection["locations"][0]["physicalLocation"]["region"]["startLine"],
        14
    );
    let rule = |id: &str| rules.iter().find(|rule| rule["id"] == id).unwrap();
    assert!(
        rule("script-injection")["properties"]["tags"]
            .as_array()
            .unwrap()
            .contains(&"security".into())
    );
    assert!(rule("script-injection")["properties"]["security-severity"].is_string());
    assert_eq!(rule("script-injection")["properties"]["precision"], "high");
    assert_eq!(rule("unpinned-action")["properties"]["precision"], "medium");
}

#[test]
fn github_actions_runs_default_to_annotations_unless_a_format_is_given() {
    let scan = |format: &[&str]| {
        let output = bin()
            .current_dir(fixture("repos/configured"))
            .env("GITHUB_ACTIONS", "true")
            .args(["--persona", "pedantic", "--offline", "--no-config"])
            .args(format)
            .arg(".github/workflows")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        String::from_utf8(output.stdout).unwrap()
    };
    let annotations = scan(&[]);
    assert!(
        annotations.lines().all(|line| line.starts_with("::")),
        "{annotations}"
    );
    assert!(annotations.contains("::error file=.github/workflows/ci.yml,line=14,"));
    serde_json::from_str::<serde_json::Value>(&scan(&["-f", "json"])).unwrap();
    assert!(scan(&["--format", "plain"]).contains("[script-injection]"));
}

#[test]
fn rule_catalog_as_json_matches_the_text_catalog() {
    let list = |format: &[&str]| {
        let output = bin()
            .env("GITHUB_ACTIONS", "true")
            .arg("--list-rules")
            .args(format)
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    let text = list(&[]);
    let json: serde_json::Value = serde_json::from_str(&list(&["-f", "json"])).unwrap();
    let rules = json["rules"].as_array().unwrap();
    assert_eq!(rules.len(), text.lines().count());
    for (rule, line) in rules.iter().zip(text.lines()) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        for (key, value) in ["id", "tag", "severity", "persona", "scope"]
            .iter()
            .zip(fields)
        {
            assert_eq!(rule[key], value, "{rule}");
        }
        assert!(!rule["summary"].as_str().unwrap().is_empty(), "{rule}");
    }
    let injection = rules
        .iter()
        .find(|rule| rule["id"] == "script-injection")
        .unwrap();
    assert_eq!(injection["equivalents"][0]["rule"], "template-injection");

    let rejected = bin()
        .args(["--list-rules", "--format", "sarif"])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
}

#[test]
fn json_report_after_fix_lists_what_remains() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("crosgha-json-fix-{unique}.yml"));
    fs::copy(fixture("checkout-credentials.before.yml"), &path).unwrap();
    let output = bin()
        .args(["--offline", "--fix", "--format", "json"])
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(&path).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["summary"]["fixed"], 3);
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(json["findings"].as_array().unwrap().len(), 0);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn zizmor_ids_for_part_of_a_rule_only_touch_that_part() {
    let output = bin()
        .args(["--persona", "pedantic", "--offline", "--fail-on", "none"])
        .arg(fixture("repos/partial/.github/workflows"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines = |rule: &str| -> Vec<String> {
        diagnostics(&stdout, rule)
            .into_iter()
            .map(|(_, at)| {
                at.split("ci.yml:")
                    .nth(1)
                    .unwrap()
                    .split(':')
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    };
    // `unpinned-images` covers container images only; action refs stay checked.
    assert_eq!(lines("unpinned-action"), ["13"], "{stdout}");
    // `unsound-condition` covers text around `${{ }}`, not spoofable bot names.
    assert_eq!(lines("unsafe-if-condition"), ["14"], "{stdout}");
}

#[test]
fn github_annotations_and_a_sarif_file_come_from_one_run() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sarif_path = std::env::temp_dir().join(format!("crosgha-{unique}.sarif"));
    let output = bin()
        .current_dir(fixture("repos/configured"))
        .args([
            "--persona",
            "pedantic",
            "--offline",
            "--no-config",
            "--format",
            "github",
        ])
        .arg("--sarif-file")
        .arg(&sarif_path)
        .arg(".github/workflows")
        .output()
        .unwrap();
    let sarif: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sarif_path).unwrap()).unwrap();
    fs::remove_file(&sarif_path).unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let annotations: Vec<&str> = stdout.lines().collect();
    assert!(
        annotations
            .iter()
            .all(|line| ["::error ", "::warning ", "::notice "]
                .iter()
                .any(|kind| line.starts_with(kind))),
        "{stdout}"
    );
    assert_eq!(
        annotations.len(),
        sarif["runs"][0]["results"].as_array().unwrap().len()
    );
    let injection = annotations
        .iter()
        .find(|line| line.contains("title=script-injection::"))
        .unwrap();
    assert!(
        injection.starts_with("::error file=.github/workflows/ci.yml,line=14,endLine=14,col="),
        "{injection}"
    );
    assert!(
        injection.ends_with("(zizmor: template-injection; actionlint: expression)"),
        "{injection}"
    );
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
fn ignore_entries_with_a_malformed_position_are_config_errors() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("crosgha-ignore-{}-{unique}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let scan = |entry: &str| {
        let config = root.join("zizmor.yml");
        fs::write(
            &config,
            format!("rules:\n  template-injection:\n    ignore:\n      - \"{entry}\"\n"),
        )
        .unwrap();
        bin()
            .args(["--offline", "--fail-on", "none", "--config"])
            .arg(&config)
            .arg(fixture("repos/configured/.github/workflows"))
            .output()
            .unwrap()
    };
    // Line 14 is one of two script injections in ci.yml; a bad position must not hide both.
    for entry in [
        "ci.yml:L14",
        "ci.yml:",
        "ci.yml:14x",
        "ci.yml:14:x",
        "ci.yml:14:9:1",
    ] {
        let output = scan(entry);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.code(), Some(2), "{entry}: {stderr}");
        assert!(stderr.contains(&format!("`{entry}`")), "{entry}: {stderr}");
    }
    let valid = scan("ci.yml:14:9");
    assert!(valid.status.success());
    let report = String::from_utf8(valid.stdout).unwrap();
    let injections = diagnostics(&report, "script-injection");
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(injections.len(), 1, "{report}");
    assert!(injections[0].1.contains("ci.yml:15:"), "{report}");
}

#[test]
fn a_workflow_named_zizmor_yml_is_never_read_as_the_config() {
    // The root `zizmor.yml` disables artipacked; `.github/workflows/zizmor.yml` runs zizmor.
    let repo = fixture("repos/zizmor-workflow");
    let scan = |extra: &[&str], input: &PathBuf| {
        let output = bin()
            .args(["--offline", "--fail-on", "none"])
            .args(extra)
            .arg(input)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let workflow = repo.join(".github/workflows/ci.yml");
    for input in [&repo, &repo.join(".github/workflows"), &workflow] {
        let report = scan(&[], input);
        assert!(
            diagnostics(&report, "persist-credentials").is_empty(),
            "{input:?}: {report}"
        );
    }
    let unconfigured = scan(&["--no-config"], &workflow);
    assert_eq!(
        diagnostics(&unconfigured, "persist-credentials").len(),
        1,
        "{unconfigured}"
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
fn each_input_uses_its_own_config_whatever_the_order() {
    // Only `configured` has a config: it disables unpinned-uses and raises artipacked to high.
    let scan = |inputs: &[&str]| {
        let output = bin()
            .current_dir(fixture("repos"))
            .args(["--offline", "--persona", "pedantic", "--fail-on", "none"])
            .args(["-f", "agent"])
            .args(inputs)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut lines: Vec<String> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        lines.sort();
        lines
    };
    let mut alone = [scan(&["configured"]), scan(&["insecure"])].concat();
    alone.sort();
    assert!(
        alone
            .iter()
            .any(|line| line.contains("error persist-credentials"))
            && !alone.iter().any(|line| line.contains("unpinned-action"))
            && alone.iter().any(|line| line.starts_with("insecure/")),
        "{alone:#?}"
    );
    assert_eq!(scan(&["configured", "insecure"]), alone);
    assert_eq!(scan(&["insecure", "configured"]), alone);
}

/// A throwaway repository whose git ignores the user's global and system config (hooksPath, signing).
fn git_repo(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("crosgha-hook-{name}-{unique}"));
    fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q", "--bare", "remote.git"]);
    git(&root, &["init", "-q", "work"]);
    root.join("work")
}

fn git(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let bin_dir = PathBuf::from(env!("CARGO_BIN_EXE_crosgha"))
        .parent()
        .unwrap()
        .to_path_buf();
    let path = std::env::join_paths(
        std::iter::once(bin_dir).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("PATH", path)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_ACTIONS")
        .output()
        .unwrap()
}

fn combined(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn pre_push_hook_blocks_smelly_workflows_until_they_are_fixed() {
    let work = git_repo("push");
    // Spaces and quotes in the scanned path prove the stored arguments survive shell quoting.
    let flows = work.join("ci flows");
    fs::create_dir_all(&flows).unwrap();
    fs::copy(
        fixture("checkout-credentials.before.yml"),
        flows.join("it's.yml"),
    )
    .unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-qm", "ci"]);
    fs::create_dir_all(work.join("src")).unwrap();

    let install = bin()
        .current_dir(work.join("src"))
        .args(["hook", "install", "--", "--offline", "ci flows"])
        .output()
        .unwrap();
    assert!(install.status.success(), "{}", combined(&install));
    let hook = work.join(".git/hooks/pre-push");
    assert!(
        combined(&install).contains("pre-push"),
        "{}",
        combined(&install)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(fs::metadata(&hook).unwrap().permissions().mode() & 0o111, 0);
    }

    let remote = work.parent().unwrap().join("remote.git");
    let remote_url = remote.to_str().unwrap();
    let blocked = git(
        &work.join("src"),
        &["push", remote_url, "HEAD:refs/heads/main"],
    );
    assert!(!blocked.status.success(), "{}", combined(&blocked));
    let report = combined(&blocked);
    assert!(report.contains("[persist-credentials]"), "{report}");
    assert!(report.contains("crosgha --fix"), "{report}");
    assert!(
        !git(&remote, &["rev-parse", "--verify", "-q", "main"])
            .status
            .success()
    );

    // Without crosgha on PATH the hook fails closed instead of letting the push through.
    let missing = Command::new("git")
        .current_dir(&work)
        .args(["push", remote_url, "HEAD:refs/heads/main"])
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(
        combined(&missing).contains("crosgha is not on PATH"),
        "{}",
        combined(&missing)
    );

    let fixed = bin()
        .current_dir(&work)
        .args(["--offline", "--fix", "ci flows"])
        .output()
        .unwrap();
    assert!(fixed.status.success(), "{}", combined(&fixed));
    git(&work, &["commit", "-qam", "fix"]);
    let pushed = git(&work, &["push", remote_url, "HEAD:refs/heads/main"]);
    assert!(pushed.status.success(), "{}", combined(&pushed));
    assert!(
        git(&remote, &["rev-parse", "--verify", "-q", "main"])
            .status
            .success()
    );

    let uninstall = bin()
        .current_dir(&work)
        .args(["hook", "uninstall"])
        .output()
        .unwrap();
    assert!(uninstall.status.success(), "{}", combined(&uninstall));
    assert!(!hook.exists());
    fs::remove_dir_all(work.parent().unwrap()).unwrap();
}

#[test]
fn hook_install_follows_hooks_path_and_never_touches_foreign_hooks() {
    let work = git_repo("foreign");
    git(&work, &["config", "core.hooksPath", ".githooks"]);
    fs::create_dir_all(work.join(".githooks")).unwrap();
    let foreign = work.join(".githooks/pre-push");
    // Not valid UTF-8: a hook we cannot even read as text is still someone else's.
    fs::write(&foreign, b"#!/bin/sh\nmake check # \xff\n").unwrap();

    for command in [&["hook", "install"][..], &["hook", "uninstall"][..]] {
        let refused = bin().current_dir(&work).args(command).output().unwrap();
        assert_eq!(refused.status.code(), Some(2), "{}", combined(&refused));
        assert!(
            combined(&refused).contains(".githooks/pre-push"),
            "{}",
            combined(&refused)
        );
        assert_eq!(
            fs::read(&foreign).unwrap(),
            b"#!/bin/sh\nmake check # \xff\n"
        );
    }
    assert!(!work.join(".git/hooks/pre-push").exists());

    fs::remove_file(&foreign).unwrap();
    std::os::unix::fs::symlink("/nonexistent/pre-push", &foreign).unwrap();
    let dangling = bin()
        .current_dir(&work)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert_eq!(dangling.status.code(), Some(2), "{}", combined(&dangling));
    assert!(fs::symlink_metadata(&foreign).unwrap().is_symlink());

    let typo = bin()
        .current_dir(&work)
        .args(["hook", "install", "--", "--persona", "paranoid"])
        .output()
        .unwrap();
    assert_eq!(typo.status.code(), Some(2), "{}", combined(&typo));
    assert!(combined(&typo).contains("paranoid"), "{}", combined(&typo));

    fs::remove_file(&foreign).unwrap();
    for persona in ["pedantic", "auditor"] {
        let installed = bin()
            .current_dir(&work)
            .args(["hook", "install", "--", "--persona", persona])
            .output()
            .unwrap();
        assert!(installed.status.success(), "{}", combined(&installed));
    }
    let hook = fs::read_to_string(&foreign).unwrap();
    assert!(
        hook.contains("'auditor'") && !hook.contains("pedantic"),
        "{hook}"
    );

    let outside = std::env::temp_dir();
    let no_repo = bin()
        .current_dir(&outside)
        .env("GIT_CEILING_DIRECTORIES", &outside)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert_eq!(no_repo.status.code(), Some(2), "{}", combined(&no_repo));
    fs::remove_dir_all(work.parent().unwrap()).unwrap();
}

#[test]
fn text_report_points_at_the_source_and_colors_only_on_request() {
    let scan = |extra: &[&str]| {
        let output = bin()
            .env_remove("NO_COLOR")
            .env_remove("CLICOLOR_FORCE")
            .args(extra)
            .arg(fixture("checkout-credentials.before.yml"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        String::from_utf8(output.stdout).unwrap()
    };
    let piped = scan(&[]);
    assert!(!piped.contains('\x1b'), "{piped}");
    let lines: Vec<&str> = piped.lines().collect();
    let header = lines
        .iter()
        .position(|line| line.starts_with("warning[persist-credentials]: "))
        .unwrap();
    assert!(lines[header + 1].ends_with("checkout-credentials.before.yml:9:9"));
    let (source, carets) = (lines[header + 3], lines[header + 4]);
    assert!(
        source.ends_with(
            "- uses: actions/checkout@fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09 # v5.1.0"
        )
    );
    assert_eq!(carets.find('^'), source.find("uses"), "{piped}");
    assert!(carets.ends_with("^^^^ job `test`, step 1"), "{piped}");
    assert!(piped.contains("Checked 1 workflow file: 3 findings (3 warnings).\n"));
    assert!(
        piped.ends_with("3 findings have a fix: rerun with --fix.\n"),
        "{piped}"
    );

    let colored = scan(&["--color", "always"]);
    assert!(
        colored.contains("\x1b[1m\x1b[33mwarning[persist-credentials]"),
        "{colored}"
    );
    assert_eq!(anstream::adapter::strip_str(&colored).to_string(), piped);
}
