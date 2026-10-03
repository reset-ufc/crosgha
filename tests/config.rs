//! crosgha.yml and zizmor.yml: discovery, per-input configs, ignores, and schema errors.

mod common;

use common::{TempDir, bin, diagnostics, fixture};
use std::fs;
use std::path::PathBuf;

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
        // An empty variable is unset, so the config is discovered.
        let empty = bin()
            .args(["--persona", "pedantic", "--offline", "--fail-on", "none"])
            .env(variable, "")
            .arg(&workflows)
            .output()
            .unwrap();
        assert!(
            empty.status.success(),
            "{variable}: {}",
            String::from_utf8_lossy(&empty.stderr)
        );
        assert_eq!(String::from_utf8(empty.stdout).unwrap(), configured);
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
fn ignore_entries_with_a_malformed_position_are_config_errors() {
    let temp = TempDir::new("ignore");
    let root = temp.join("work");
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
    assert_eq!(injections.len(), 1, "{report}");
    assert!(injections[0].1.contains("ci.yml:15:"), "{report}");
}

#[test]
fn config_keys_with_a_wrong_type_or_an_unknown_name_are_config_errors() {
    let temp = TempDir::new("config");
    let root = temp.join("work");
    fs::create_dir_all(&root).unwrap();
    let config = root.join("zizmor.yml");
    let scan = |text: &str| {
        fs::write(&config, text).unwrap();
        bin()
            .args(["--offline", "--fail-on", "none", "--config"])
            .arg(&config)
            .arg(fixture("repos/configured/.github/workflows"))
            .output()
            .unwrap()
    };
    // Each typo would otherwise be dropped silently and leave the rule enabled.
    let broken = [
        (
            "rules:\n  artipacked:\n    ignore: ci.yml\n",
            "rules.artipacked.ignore",
        ),
        (
            "rules:\n  artipacked:\n    ignore: [4]\n",
            "rules.artipacked.ignore",
        ),
        (
            "rules:\n  artipacked:\n    disable: \"true\"\n",
            "rules.artipacked.disable",
        ),
        (
            "rules:\n  artipacked:\n    disabled: true\n",
            "rules.artipacked: unknown field `disabled`",
        ),
        (
            "rules:\n  artipacked:\n    remap:\n      severity: hihg\n",
            "rules.artipacked.remap: `severity` must be",
        ),
        (
            "rules:\n  artipacked:\n    remap:\n      level: high\n",
            "rules.artipacked.remap: unknown field `level`",
        ),
        (
            "rules:\n  artipacked:\n    config: 3\n",
            "rules.artipacked.config",
        ),
        ("rules:\n  artipacked: true\n", "rules.artipacked"),
        ("rules:\n  - artipacked\n", "rules: invalid type: sequence"),
        ("rule:\n  artipacked:\n    disable: true\n", "`rule`"),
        ("- rules\n", "mapping"),
    ];
    let outputs: Vec<_> = broken.iter().map(|(text, _)| scan(text)).collect();
    // An empty file, or an empty `rules`, configures nothing.
    let empty = [scan(""), scan("rules:\n"), scan("rules:\n  artipacked:\n")];

    for ((text, key), output) in broken.iter().zip(outputs) {
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.code(), Some(2), "{text}: {stderr}");
        assert!(stderr.contains(key), "{text}: {stderr}");
    }
    for output in empty {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
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

#[test]
fn the_config_depends_on_the_repository_not_on_how_inputs_are_spelled() {
    // The root `crosgha.yml` disables persist-credentials; `.github/zizmor.yml` does not, and
    // crosgha's own file wins over zizmor's wherever each sits.
    let temp = TempDir::new("config");
    let repo = temp.join("work");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir_all(repo.join(".github/workflows")).unwrap();
    fs::copy(
        fixture("checkout-credentials.before.yml"),
        repo.join(".github/workflows/ci.yml"),
    )
    .unwrap();
    fs::write(
        repo.join("crosgha.yml"),
        "rules:\n  persist-credentials:\n    disable: true\n",
    )
    .unwrap();
    fs::write(repo.join(".github/zizmor.yml"), "rules: {}\n").unwrap();
    let scan = |inputs: &[&str]| {
        let output = bin()
            .current_dir(&repo)
            .args(["--offline", "--fail-on", "none", "-f", "agent"])
            .args(inputs)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{inputs:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let reports: Vec<(&[&str], String)> = [
        &["."][..],
        &[".github"],
        &[".github/workflows"],
        &[".github/workflows/ci.yml"],
        &[".github", ".github/workflows/ci.yml"],
        &[".github/workflows/ci.yml", ".github"],
    ]
    .into_iter()
    .map(|inputs| (inputs, scan(inputs)))
    .collect();

    for (inputs, report) in &reports {
        assert!(
            !report.contains("persist-credentials"),
            "{inputs:?}: {report}"
        );
    }
}
