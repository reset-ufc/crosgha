//! `--fix`: verified writes, rejected fixes, line breaks, BOMs, symlinks, and concurrent edits.

mod common;

use common::{TempDir, bin, fixture};
use std::fs;

#[test]
fn fixes_checkout_credentials_without_changing_other_nodes() {
    let temp = TempDir::new("test");
    let path = temp.join("workflow.yml");
    fs::copy(fixture("checkout-credentials.before.yml"), &path).unwrap();

    let output = bin().arg("--fix").arg(&path).output().unwrap();
    let actual = fs::read_to_string(&path).unwrap();

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
    let temp = TempDir::new("bom");
    let path = temp.join("workflow.yml");
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

    // The text report shows and underlines line 1 the same way with or without the BOM.
    let text = |source: &str| {
        fs::write(&path, source).unwrap();
        let output = bin()
            .args(["--persona", "auditor", "--offline", "--no-config"])
            .arg(&path)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    let plain = "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      \
                 - run: echo hi\n";
    let with_bom = text(&format!("\u{feff}{plain}"));
    let without = text(plain);
    assert!(
        without.contains("1 | on: push\n  | ^^ workflow"),
        "{without}"
    );
    assert_eq!(with_bom, without);
}

#[test]
fn fix_does_not_overwrite_invalid_workflow() {
    let temp = TempDir::new("invalid");
    let path = temp.join("workflow.yml");
    fs::copy(fixture("malformed.yml"), &path).unwrap();
    let before = fs::read(&path).unwrap();

    let output = bin().arg("--fix").arg(&path).output().unwrap();
    let after = fs::read(&path).unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(before, after);
}

#[test]
fn fixes_checkout_while_reporting_a_detection_only_finding() {
    let temp = TempDir::new("mixed");
    let path = temp.join("workflow.yml");
    fs::copy(fixture("mixed-fixes.before.yml"), &path).unwrap();

    let regular = bin().arg(&path).output().unwrap();
    let output = bin()
        .args(["--persona", "pedantic", "--fix"])
        .arg(&path)
        .output()
        .unwrap();
    let actual = fs::read_to_string(&path).unwrap();

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

/// A fix that fails validation stays a reported finding; the other fixes in that file and in the
/// other files are still written. The fix applied above the rejected one moves it from line 16 to
/// 18, and every report gives the line in the written file.
#[test]
fn rejected_fix_keeps_its_finding_and_the_other_fixes() {
    let temp = TempDir::new("reject");
    let dir = temp.join("work");
    fs::create_dir(&dir).unwrap();
    let (flow, ci) = (dir.join("flow.yml"), dir.join("ci.yml"));
    fs::copy(fixture("rejected-fix.yml"), &flow).unwrap();
    fs::copy(fixture("checkout-credentials.before.yml"), &ci).unwrap();

    let json = bin()
        .args(["--fix", "-f", "json"])
        .arg(&dir)
        .output()
        .unwrap();
    let flow_after = fs::read_to_string(&flow).unwrap();
    let ci_after = fs::read_to_string(&ci).unwrap();
    fs::copy(fixture("rejected-fix.yml"), &flow).unwrap();
    let text = bin().arg("--fix").arg(&flow).output().unwrap();

    assert_eq!(json.status.code(), Some(1), "{json:?}");
    assert_eq!(
        ci_after,
        fs::read_to_string(fixture("checkout-credentials.after.yml")).unwrap()
    );
    let original = fs::read_to_string(fixture("rejected-fix.yml")).unwrap();
    assert!(
        flow_after
            .contains("- { uses: actions/checkout@fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09 }\n"),
        "{flow_after}"
    );
    assert_eq!(
        flow_after,
        original.replace(
            "# v5.1.0\n",
            "# v5.1.0\n        with:\n          persist-credentials: false\n"
        )
    );

    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(report["summary"]["fixed"], 4, "{report}");
    let remaining = report["findings"].as_array().unwrap();
    assert_eq!(remaining.len(), 1, "{report}");
    assert_eq!(remaining[0]["rule"], "persist-credentials");
    assert!(remaining[0]["path"].as_str().unwrap().ends_with("flow.yml"));
    assert_eq!(remaining[0]["line"], 18);
    let fixes = report["fixes"].as_array().unwrap();
    let status = |status: &str| {
        fixes
            .iter()
            .filter(|fix| fix["status"] == status)
            .collect::<Vec<_>>()
    };
    assert_eq!(status("applied").len(), 4, "{report}");
    let rejected = status("rejected");
    assert_eq!(rejected.len(), 1, "{report}");
    assert_eq!(rejected[0]["line"], 18);
    assert_eq!(
        rejected[0]["reason"],
        "fix changed YAML outside the intended node"
    );

    assert_eq!(text.status.code(), Some(1));
    let stdout = String::from_utf8(text.stdout).unwrap();
    assert!(
        stdout.contains("Fixed 1 finding in 1 workflow file."),
        "{stdout}"
    );
    assert!(
        stdout.contains("flow.yml:18:11 [persist-credentials]: fix changed YAML outside"),
        "{stdout}"
    );
    assert_eq!(
        stdout.matches("[persist-credentials]:").count(),
        2,
        "{stdout}"
    );
    assert!(!stdout.contains("--fix"), "{stdout}");
}

/// Layouts common in real repositories: a comment (here CJK) after the fixed step, CRLF or mixed
/// line breaks, and no final newline. The fix keeps each line's break and the missing final
/// newline.
#[test]
fn fixes_workflows_with_trailing_comments_crlf_and_no_final_newline() {
    let temp = TempDir::new("layout");
    let dir = temp.join("work");
    fs::create_dir(&dir).unwrap();
    let source = fs::read_to_string(fixture("fix-layout.yml")).unwrap();
    let unterminated = source.trim_end_matches('\n');
    let crlf = source.replace('\n', "\r\n");
    // Editors that mix line breaks: odd lines end with CRLF, even lines with LF.
    let ending = |index: usize| {
        if index.is_multiple_of(2) {
            "\r\n"
        } else {
            "\n"
        }
    };
    let mut mixed = String::new();
    let mut fixed_mixed_expected = String::new();
    for (index, line) in source.lines().enumerate() {
        let end = ending(index);
        mixed.push_str(&format!("{line}{end}"));
        fixed_mixed_expected.push_str(&format!("{line}{end}"));
        // Each inserted line takes the line break of the line it follows.
        if line.ends_with("# v5.1.0") {
            fixed_mixed_expected.push_str(&format!(
                "        with:{end}          persist-credentials: false{end}"
            ));
        }
    }
    fs::write(dir.join("unterminated.yml"), unterminated).unwrap();
    fs::write(dir.join("crlf.yml"), &crlf).unwrap();
    fs::write(dir.join("mixed.yml"), &mixed).unwrap();

    let output = bin().arg("--fix").arg(&dir).output().unwrap();
    let fixed_unterminated = fs::read_to_string(dir.join("unterminated.yml")).unwrap();
    let fixed_crlf = fs::read_to_string(dir.join("crlf.yml")).unwrap();
    let fixed_mixed = fs::read_to_string(dir.join("mixed.yml")).unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(0), "{stdout}");
    assert!(
        stdout.contains("Fixed 6 findings in 3 workflow files."),
        "{stdout}"
    );
    let with = "# v5.1.0\n        with:\n          persist-credentials: false\n";
    let expected = source.replace("# v5.1.0\n", with);
    assert_eq!(fixed_unterminated, expected.trim_end_matches('\n'));
    assert_eq!(fixed_crlf, expected.replace('\n', "\r\n"));
    assert_eq!(fixed_mixed, fixed_mixed_expected);
}

/// A workflow saved while the scan runs keeps the new content: its fix was computed from the old
/// one, so it is reported as not written and the run fails. The edit lands while online checks
/// wait for the API, which answers every request with 404.
#[test]
fn a_workflow_edited_during_the_scan_is_not_overwritten() {
    use std::io::{BufRead, BufReader, Write};
    let temp = TempDir::new("edited");
    let path = temp.join("workflow.yml");
    let source = "on: push\npermissions: {}\njobs:\n  build:\n    runs-on: ubuntu-latest\n    \
                  timeout-minutes: 5\n    steps:\n      \
                  - uses: actions/checkout@fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09 # v5.1.0\n";
    let edited = format!("{source}      - run: make\n");
    fs::write(&path, source).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let api = format!("http://{}", listener.local_addr().unwrap());
    let (target, content) = (path.clone(), edited.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 && line != "\r\n" {
                line.clear();
            }
            fs::write(&target, &content).unwrap();
            let body = r#"{"message":"Not Found"}"#;
            let _ = write!(
                stream,
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    let sarif = path.with_extension("sarif");
    let output = bin()
        .args(["--fix", "--no-config", "-f", "json", "--gh-token", "t"])
        .args(["--gh-hostname", &api])
        .arg("--sarif-file")
        .arg(&sarif)
        .arg(&path)
        .output()
        .unwrap();
    let after = fs::read_to_string(&path).unwrap();
    let sarif: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sarif).unwrap()).unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(after, edited);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("file changed since it was read"),
        "{stderr}"
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let fix = &report["fixes"][0];
    assert_eq!(fix["rule"], "persist-credentials", "{report:#}");
    assert_eq!(fix["status"], "not-written", "{report:#}");
    assert_eq!(
        fix["reason"], "file changed since it was read",
        "{report:#}"
    );
    // The failed write is a run error in every machine-readable format, not a success.
    assert_eq!(report["summary"]["errors"], 1, "{report:#}");
    let error = &report["errors"][0];
    assert_eq!(error["message"], "file changed since it was read");
    assert!(error["path"].as_str().unwrap().ends_with(".yml"), "{error}");
    let invocation = &sarif["runs"][0]["invocations"][0];
    assert_eq!(invocation["executionSuccessful"], false, "{invocation:#}");
    assert_eq!(
        invocation["toolExecutionNotifications"][0]["message"]["text"],
        "file changed since it was read"
    );
}

/// Fixing one of two equal checkout steps makes them differ until the other is fixed too, which
/// clears and restores `duplicated-steps` (pedantic and auditor only). The fixes must not depend
/// on the persona.
#[test]
fn fixes_of_equal_steps_apply_under_every_persona() {
    let temp = TempDir::new("equal");
    let source = "on: push\npermissions: {}\njobs:\n  build:\n    runs-on: ubuntu-latest\n    \
                  timeout-minutes: 5\n    steps:\n      - uses: actions/checkout@v4\n      \
                  - run: ./cleanup.sh\n      - uses: actions/checkout@v4\n";
    let with = "@v4\n        with:\n          persist-credentials: false\n";
    for persona in ["regular", "pedantic", "auditor"] {
        let path = temp.join(format!("{persona}.yml"));
        fs::write(&path, source).unwrap();
        let output = bin()
            .args(["--fix", "--offline", "--no-config", "-f", "json"])
            .args(["--fail-on", "none", "--persona", persona])
            .arg(&path)
            .output()
            .unwrap();
        let fixed = fs::read_to_string(&path).unwrap();

        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["summary"]["fixed"], 2, "{persona}: {report:#}");
        assert_eq!(fixed, source.replace("@v4\n", with), "{persona}");
        let duplicated = report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["rule"] == "duplicated-steps")
            .count();
        assert_eq!(duplicated, usize::from(persona != "regular"), "{report:#}");
    }
}

/// Trailing spaces after the node a key is added to stay on their own line: the added lines go
/// between whole lines, so every existing line stays byte for byte.
#[test]
fn added_lines_leave_trailing_whitespace_on_its_line() {
    let temp = TempDir::new("trailing");
    let path = temp.join("workflow.yml");
    let source = "on: push\npermissions: {}\njobs:\n  build:\n    runs-on: ubuntu-latest\n    \
                  timeout-minutes: 5\n    steps:\n      - uses: actions/checkout@v4\n        \
                  with:\n          fetch-depth: 0 \t \n      - run: make\n";
    fs::write(&path, source).unwrap();
    let output = bin()
        .args(["--fix", "--offline", "--no-config", "-f", "json"])
        .arg(&path)
        .output()
        .unwrap();
    let fixed = fs::read_to_string(&path).unwrap();

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["fixes"][0]["status"], "applied", "{report:#}");
    assert_eq!(
        fixed,
        source.replace("0 \t \n", "0 \t \n          persist-credentials: false\n")
    );
}

/// A fix is never written through a symbolic link, but a link with nothing to fix is fine and
/// the regular files next to it are still fixed.
#[cfg(unix)]
#[test]
fn fixes_regular_workflows_but_refuses_to_write_through_symlinks() {
    let temp = TempDir::new("links");
    let root = temp.join("work");
    let scanned = root.join("workflows");
    fs::create_dir_all(&scanned).unwrap();
    let (shared, clean) = (root.join("shared.yml"), root.join("clean.yml"));
    fs::copy(fixture("checkout-credentials.before.yml"), &shared).unwrap();
    fs::copy(fixture("workflows/ci.yml"), &clean).unwrap();
    fs::copy(fixture("workflows/ci.yml"), scanned.join("ci.yml")).unwrap();
    let real = scanned.join("real.yml");
    fs::copy(fixture("checkout-credentials.before.yml"), &real).unwrap();
    std::os::unix::fs::symlink(&shared, scanned.join("linked.yml")).unwrap();
    std::os::unix::fs::symlink(&clean, scanned.join("clean-link.yml")).unwrap();
    // A link to another scanned workflow is that workflow: reported and fixed once, by its target.
    std::os::unix::fs::symlink("real.yml", scanned.join("alias.yml")).unwrap();
    let before = fs::read(&shared).unwrap();

    let scan = bin().arg(&scanned).output().unwrap();
    let fix = bin().arg("--fix").arg(&scanned).output().unwrap();
    let after = fs::read(&shared).unwrap();
    let fixed = fs::read_to_string(&real).unwrap();
    let still_linked = fs::symlink_metadata(scanned.join("linked.yml"))
        .unwrap()
        .is_symlink();
    let clean_fix = bin()
        .arg("--fix")
        .arg(scanned.join("clean-link.yml"))
        .output()
        .unwrap();

    assert_eq!(scan.status.code(), Some(1));
    let stdout = String::from_utf8(scan.stdout).unwrap();
    assert!(
        stdout.contains("Checked 4 workflow files: 6 findings"),
        "{stdout}"
    );
    assert!(stdout.contains("linked.yml"));
    assert!(!stdout.contains("alias.yml"), "{stdout}");

    assert_eq!(fix.status.code(), Some(2));
    let stderr = String::from_utf8(fix.stderr).unwrap();
    assert_eq!(
        stderr
            .matches("refusing to replace a symbolic link")
            .count(),
        1,
        "{stderr}"
    );
    assert!(stderr.contains("linked.yml"), "{stderr}");
    assert_eq!(before, after);
    assert!(still_linked);
    assert_eq!(
        fixed,
        fs::read_to_string(fixture("checkout-credentials.after.yml")).unwrap()
    );
    let stdout = String::from_utf8(fix.stdout).unwrap();
    assert!(
        stdout.contains("Fixed 3 findings in 1 workflow file (1 failed)."),
        "{stdout}"
    );
    assert_eq!(
        stdout
            .matches("[persist-credentials]: refusing to replace a symbolic link")
            .count(),
        3,
        "{stdout}"
    );
    assert!(stdout.contains("Remaining: 3 findings"), "{stdout}");
    assert!(!stdout.contains("alias.yml"), "{stdout}");

    assert_eq!(
        clean_fix.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&clean_fix.stderr)
    );
}
