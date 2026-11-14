#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! `crosgha migrate --from zizmor|actionlint|poutine|ghalint|octoscan` on workflows and repositories.

mod common;

use common::{TempDir, bin, copy_tree, fixture};
use std::fs;

#[test]
fn migrate_replaces_other_scanners_with_the_crosgha_action() {
    let action = format!("reset-ufc/crosgha@{}", "c".repeat(40));
    for tool in ["zizmor", "actionlint", "poutine", "octoscan"] {
        let temp = TempDir::new("migrate");
        let path = temp.join("workflow.yml");
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
        if tool == "octoscan" {
            let stderr = String::from_utf8_lossy(&output.stderr);
            for (line, reason) in [
                ("workflow.yml:48", "`disable_rules` names octoscan rules"),
                (
                    "workflow.yml:51",
                    "`config_file` points to octoscan's config",
                ),
                (
                    "workflow.yml:61",
                    "reads `steps.scan.outputs.sarif_output`, which octoscan wrote",
                ),
            ] {
                assert!(
                    stderr
                        .lines()
                        .any(|note| note.contains(line) && note.contains(reason)),
                    "{line} {reason}: {stderr}"
                );
            }
        }
    }
}

#[test]
fn migrate_converts_configs_and_shell_steps() {
    let action = format!("reset-ufc/crosgha@{}", "c".repeat(40));
    for (tool, workflow, config) in [
        ("zizmor", "ci.yml", Some("zizmor.yml")),
        ("actionlint", "lint.yml", Some("actionlint.yaml")),
        ("poutine", "scan.yml", Some("poutine.yml")),
        ("ghalint", "ghalint.yml", Some("ghalint.yaml")),
        // octoscan reads a config only from `--config-file`.
        ("octoscan", "scan.yml", None),
    ] {
        let temp = TempDir::new("migrate");
        let repo = temp.join("work");
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
        let original =
            config.map(|config| fs::read_to_string(repo.join(".github").join(config)).unwrap());
        let scan = bin()
            .args(["--offline", "--fail-on", "none"])
            .arg(repo.join(".github/workflows"))
            .output()
            .unwrap();

        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{tool}: {stderr}");
        // No expected crosgha.yml means the migration writes none.
        let expected = |name: &str| {
            fs::read_to_string(fixture(&format!("migrate/expected/{tool}-repo.{name}")))
                .unwrap_or_default()
        };
        assert_eq!(migrated, expected(workflow), "{tool}");
        assert_eq!(crosgha, expected("crosgha.yml"), "{tool}");
        assert_eq!(
            original,
            config.map(|config| {
                fs::read_to_string(fixture(&format!("migrate/{tool}-repo/.github/{config}")))
                    .unwrap()
            }),
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
                assert!(stderr.contains("`artipacked` was not renamed"), "{stderr}");
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
                // Chains, redirects, and substitutions inside a word cannot become Action inputs.
                for line in ["ci.yml:52", "ci.yml:53", "ci.yml:54"] {
                    assert!(
                        stderr
                            .lines()
                            .any(|note| note.contains(line) && note.contains("shell syntax")),
                        "{line}: {stderr}"
                    );
                }
                // No crosgha step replaced zizmor in this job, so whatever runs it still needs it.
                assert!(
                    stderr
                        .lines()
                        .any(|note| note.contains("ci.yml:59") && note.contains("was kept")),
                    "{stderr}"
                );
            }
            "actionlint" => {
                assert!(stderr.contains("`config-variables`"), "{stderr}");
                assert!(stderr.contains("`paths`"), "{stderr}");
            }
            "ghalint" => {
                // ghalint has no Action of its own, so only its shell steps move.
                assert!(!repo.join(".github/crosgha.yml").exists());
                for (line, reason) in [
                    ("ghalint.yaml", "`excludes`"),
                    (
                        "ghalint.yml:34",
                        "`ghalint run-action` without files checks every action file",
                    ),
                    ("ghalint.yml:33", "was kept"),
                    ("ghalint.yml:39", "`-c` has no crosgha Action input"),
                    (
                        "ghalint.yml:40",
                        "`ghalint exp` has no crosgha Action equivalent",
                    ),
                    (
                        "ghalint.yml:41",
                        "`GHALINT_CONFIG` has no crosgha Action input",
                    ),
                ] {
                    assert!(
                        stderr
                            .lines()
                            .any(|note| note.contains(line) && note.contains(reason)),
                        "{line} {reason}: {stderr}"
                    );
                }
            }
            "octoscan" => {
                assert!(!repo.join(".github/crosgha.yml").exists());
                for (line, reason) in [
                    // The image runs octoscan with other words than its own CLI.
                    ("scan.yml:22", "next to other commands"),
                    ("scan.yml:21", "was kept"),
                    (
                        "scan.yml:27",
                        "`octoscan dl` downloads workflows through the GitHub API",
                    ),
                    (
                        "scan.yml:28",
                        "`--format sarif` has no crosgha Action input",
                    ),
                    ("scan.yml:29", "`--ignore` has no crosgha Action input"),
                    ("scan.yml:30", "`--disable-rules`"),
                    ("scan.yml:31", "`--enable-rules`"),
                    ("scan.yml:32", "`--config-file`"),
                    ("scan.yml:33", "`--list-rules`"),
                ] {
                    assert!(
                        stderr
                            .lines()
                            .any(|note| note.contains(line) && note.contains(reason)),
                        "{line} {reason}: {stderr}"
                    );
                }
            }
            _ => {
                // poutine's `skip` and `include` have no crosgha meaning; nothing is written.
                assert!(!repo.join(".github/crosgha.yml").exists());
                for key in ["`skip`", "`include`"] {
                    assert!(
                        stderr
                            .lines()
                            .any(|note| note.contains("poutine.yml") && note.contains(key)),
                        "{key}: {stderr}"
                    );
                }
                // Only `analyze_local` scans the checkout, as crosgha's Action does.
                for (line, reason) in [
                    (
                        "scan.yml:32",
                        "`poutine analyze_repo` has no crosgha Action equivalent",
                    ),
                    (
                        "scan.yml:33",
                        "`poutine analyze_org` has no crosgha Action equivalent",
                    ),
                    ("scan.yml:31", "was kept"),
                    (
                        "scan.yml:38",
                        "`--format sarif` has no crosgha Action input",
                    ),
                    ("scan.yml:39", "`--skip`"),
                    ("scan.yml:40", "`--config`"),
                    ("scan.yml:41", "`--scm gitlab`"),
                    ("scan.yml:42", "`--format json` has no crosgha Action input"),
                ] {
                    assert!(
                        stderr
                            .lines()
                            .any(|note| note.contains(line) && note.contains(reason)),
                        "{line} {reason}: {stderr}"
                    );
                }
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn migrate_refuses_only_the_symlinked_workflows_it_must_edit() {
    let action = format!("reset-ufc/crosgha@{}", "c".repeat(40));
    let temp = TempDir::new("migrate");
    let root = temp.join("work");
    let workflows = root.join(".github/workflows");
    fs::create_dir_all(&workflows).unwrap();
    let (plain, scanner) = (root.join("plain.yml"), root.join("scanner.yml"));
    fs::copy(fixture("workflows/ci.yml"), &plain).unwrap();
    fs::copy(fixture("migrate/zizmor.before.yml"), &scanner).unwrap();
    let ci = workflows.join("ci.yml");
    let link = workflows.join("linked.yml");
    let migrate = || {
        fs::copy(fixture("migrate/zizmor.before.yml"), &ci).unwrap();
        let output = bin()
            .args(["migrate", "--from", "zizmor", "--action", &action])
            .args(["--action-version", "v0.1.0"])
            .arg(&workflows)
            .output()
            .unwrap();
        (output, fs::read_to_string(&ci).unwrap())
    };

    // A link that needs no edit is only read.
    std::os::unix::fs::symlink(&plain, &link).unwrap();
    let (untouched_link, migrated) = migrate();
    // A link that needs an edit stops the migration before any file is written.
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&scanner, &link).unwrap();
    let (edited_link, unmigrated) = migrate();
    let scanner_after = fs::read_to_string(&scanner).unwrap();
    let still_linked = fs::symlink_metadata(&link).unwrap().is_symlink();

    assert!(
        untouched_link.status.success(),
        "{}",
        String::from_utf8_lossy(&untouched_link.stderr)
    );
    assert_eq!(
        migrated,
        fs::read_to_string(fixture("migrate/zizmor.after.yml")).unwrap()
    );
    assert_eq!(edited_link.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&edited_link.stderr);
    assert!(
        stderr.contains("linked.yml") && stderr.contains("symbolic link"),
        "{stderr}"
    );
    let original = fs::read_to_string(fixture("migrate/zizmor.before.yml")).unwrap();
    assert_eq!(unmigrated, original);
    assert_eq!(scanner_after, original);
    assert!(still_linked);
}
