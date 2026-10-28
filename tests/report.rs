#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! The text, JSON, SARIF, annotation, and agent reports, and the rule catalog.

mod common;

use common::{TempDir, bin, fixture};
use std::fs;
use std::path::PathBuf;

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

/// SARIF regions count UTF-16 code units (`columnKind`), file names become percent-encoded URI
/// references, and a repository-wide finding points at the first line of the first workflow of
/// its `.github` directory, since code scanning shows no result without a location.
#[test]
fn sarif_uses_utf16_columns_encoded_uris_and_a_workflow_location_for_repositories() {
    let temp = TempDir::new("sarif");
    let repo = temp.join("work");
    let workflows = repo.join(".github/workflows");
    fs::create_dir_all(&workflows).unwrap();
    fs::write(
        workflows.join("ci build#1.yml"),
        "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      \
         - { name: \"été 🚀\", uses: actions/checkout@fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09 }\n      \
         - name: Both\n        uses: ./build\n        run: make\n",
    )
    .unwrap();
    let scan = |format: &str| {
        let output = bin()
            .current_dir(&repo)
            .args([
                "--persona",
                "auditor",
                "--offline",
                "--no-config",
                "-f",
                format,
            ])
            .arg(".")
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
    let sarif = scan("sarif");
    let annotations = scan("github");
    let (json, agent, text) = (scan("json"), scan("agent"), scan("text"));
    // A file outside the working directory has no relative URI, so it gets a `file:` URI.
    let outside = bin()
        .current_dir(fixture("workflows"))
        .args([
            "--offline",
            "--no-config",
            "--fail-on",
            "none",
            "-f",
            "sarif",
        ])
        .arg(workflows.join("ci build#1.yml"))
        .output()
        .unwrap();

    let sarif: serde_json::Value = serde_json::from_str(&sarif).unwrap();
    let run = &sarif["runs"][0];
    assert_eq!(run["columnKind"], "utf16CodeUnits");
    let results = run["results"].as_array().unwrap();
    let checkout = results
        .iter()
        .find(|result| result["ruleId"] == "persist-credentials")
        .unwrap();
    let location = &checkout["locations"][0]["physicalLocation"];
    assert_eq!(
        location["artifactLocation"]["uri"],
        ".github/workflows/ci%20build%231.yml"
    );
    // `uses` follows 26 UTF-16 code units (the emoji is a surrogate pair) but 30 UTF-8 bytes.
    assert_eq!(location["region"]["startColumn"], 27, "{location}");
    assert_eq!(location["region"]["endColumn"], 31, "{location}");

    let tooling: Vec<_> = results
        .iter()
        .filter(|result| result["ruleId"] == "missing-security-tooling")
        .collect();
    assert!(!tooling.is_empty(), "{run}");
    for result in tooling {
        let location = &result["locations"][0]["physicalLocation"];
        assert_eq!(
            location["artifactLocation"]["uri"], ".github/workflows/ci%20build%231.yml",
            "{result}"
        );
        assert_eq!(location["region"]["startLine"], 1, "{result}");
        assert!(
            result["message"]["text"]
                .as_str()
                .unwrap()
                .starts_with(".github: repository: repository has no "),
            "{result}"
        );
    }

    // Repository-wide annotations point at the same workflow, since a directory is not a file.
    let tooling: Vec<&str> = annotations
        .lines()
        .filter(|line| line.contains("title=missing-security-tooling::"))
        .collect();
    assert!(!tooling.is_empty(), "{annotations}");
    for line in tooling {
        assert!(
            line.contains("file=.github/workflows/ci build#1.yml,line=1,endLine=1,"),
            "{line}"
        );
        assert!(
            line.contains("::.github: repository: repository has no "),
            "{line}"
        );
    }

    // Annotations name the file as a path, not as a URI.
    assert!(
        annotations.contains("file=.github/workflows/ci build#1.yml,line=6,"),
        "{annotations}"
    );
    // Every other format counts columns in characters, as GitHub does for annotations: `uses`
    // follows 25 characters. Annotations omit columns on a multi-line span, which
    // actions/toolkit forbids.
    assert!(
        annotations.contains(
            "file=.github/workflows/ci build#1.yml,line=6,endLine=6,col=26,endColumn=30,\
             title=persist-credentials::"
        ),
        "{annotations}"
    );
    let multi_line: Vec<&str> = annotations
        .lines()
        .filter(|line| {
            let number = |key: &str| line.split(key).nth(1)?.split(',').next();
            number(",line=") != number(",endLine=")
        })
        .collect();
    assert!(!multi_line.is_empty(), "{annotations}");
    for line in multi_line {
        assert!(!line.contains(",col="), "{line}");
        assert!(!line.contains(",endColumn="), "{line}");
    }
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    let checkout = json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["rule"] == "persist-credentials")
        .unwrap();
    assert_eq!(
        (&checkout["column"], &checkout["end_column"]),
        (&26.into(), &30.into())
    );
    assert!(
        agent.contains(".github/workflows/ci build#1.yml:6:26: warning persist-credentials:"),
        "{agent}"
    );
    assert!(
        text.contains(".github/workflows/ci build#1.yml:6:26\n"),
        "{text}"
    );

    let outside: serde_json::Value = serde_json::from_slice(&outside.stdout).unwrap();
    assert_eq!(
        outside["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        format!(
            "file://{}/.github/workflows/ci%20build%231.yml",
            repo.display()
        )
    );
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
    // The documented catalog marks the rules that read `rules.<id>.config` in its last column.
    let catalog =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/rules.md"))
            .unwrap();
    let documented: Vec<(String, bool)> = catalog
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            (cells.len() == 7 && cells[0].starts_with('`'))
                .then(|| (cells[0].trim_matches('`').to_owned(), !cells[6].is_empty()))
        })
        .collect();
    let listed: Vec<(String, bool)> = rules
        .iter()
        .map(|rule| {
            let configurable = rule["configurable"].as_bool().expect("configurable");
            (rule["id"].as_str().unwrap().to_owned(), configurable)
        })
        .collect();
    assert_eq!(documented, listed);

    let rejected = bin()
        .args(["--list-rules", "--format", "sarif"])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
}

#[test]
fn json_report_after_fix_lists_what_remains() {
    let temp = TempDir::new("json");
    let path = temp.join("workflow.yml");
    fs::copy(fixture("checkout-credentials.before.yml"), &path).unwrap();
    let output = bin()
        .args(["--offline", "--fix", "--format", "json"])
        .arg(&path)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["summary"]["fixed"], 3);
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(json["findings"].as_array().unwrap().len(), 0);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn github_annotations_and_a_sarif_file_come_from_one_run() {
    let temp = TempDir::new("test");
    let sarif_path = temp.join("report.sarif");
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
