//! `crosgha migrate --from zizmor|actionlint` on workflows and repositories.

mod common;

use common::{TempDir, bin, fixture};
use std::fs;

#[test]
fn migrate_replaces_other_scanners_with_the_crosgha_action() {
    let action = format!("reset-ufc/crosgha@{}", "c".repeat(40));
    for tool in ["zizmor", "actionlint"] {
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
    }
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
    let action = format!("reset-ufc/crosgha@{}", "c".repeat(40));
    for (tool, workflow, config) in [
        ("zizmor", "ci.yml", "zizmor.yml"),
        ("actionlint", "lint.yml", "actionlint.yaml"),
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
        let original = fs::read_to_string(repo.join(".github").join(config)).unwrap();
        let scan = bin()
            .args(["--offline", "--fail-on", "none"])
            .arg(repo.join(".github/workflows"))
            .output()
            .unwrap();

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
            _ => {
                assert!(stderr.contains("`config-variables`"), "{stderr}");
                assert!(stderr.contains("`paths`"), "{stderr}");
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
