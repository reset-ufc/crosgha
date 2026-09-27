use std::path::{Path, PathBuf};

use clap::ValueEnum;

use serde_json::{Value, json};

use crate::{
    engine::Inspection,
    rules::{self, Finding, Located, Rule},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Format {
    /// Human-readable lines; `plain` is zizmor's name for it.
    #[value(alias = "plain")]
    Text,
    Json,
    Sarif,
    /// GitHub workflow command annotations, zizmor's `github` format.
    Github,
    /// One brief line per finding for AI agents.
    Agent,
}

pub fn render(
    format: Format,
    files: &[FileReport],
    repository: &[Located],
    errors: &[FileError],
    fix: bool,
) -> String {
    match format {
        Format::Text => text(files, repository, fix),
        Format::Json => json(files, repository, errors, fix),
        Format::Sarif => sarif(files, repository, errors, fix),
        Format::Github => github(files, repository, fix),
        Format::Agent => agent(files, repository, fix),
    }
}

pub struct FileReport {
    pub path: PathBuf,
    pub inspection: Inspection,
}

/// A workflow that could not be read or parsed; the text formats print it on stderr as it
/// happens, JSON and SARIF carry it so CI consumers see the failure.
pub struct FileError {
    pub path: PathBuf,
    pub message: String,
}

fn text(files: &[FileReport], repository: &[Located], fix: bool) -> String {
    let mut output = String::new();
    let append_repository = |output: &mut String| {
        for located in repository {
            append_findings(
                output,
                &located.path,
                std::slice::from_ref(&located.finding),
            );
        }
    };
    if fix {
        let fixed: usize = files
            .iter()
            .filter_map(|file| file.inspection.fix.as_ref())
            .map(|outcome| outcome.fixed_count)
            .sum();
        let remaining = repository.len()
            + files
                .iter()
                .filter_map(|file| file.inspection.fix.as_ref())
                .map(|outcome| outcome.remaining.len())
                .sum::<usize>();
        let changed = files
            .iter()
            .filter(|file| {
                file.inspection
                    .fix
                    .as_ref()
                    .is_some_and(|outcome| outcome.fixed_count > 0)
            })
            .count();
        output.push_str(&format!(
            "Fixed {fixed} finding(s) in {changed} workflow file(s).\n"
        ));
        if remaining > 0 {
            output.push_str(&format!("Remaining {remaining} finding(s).\n"));
            for file in files {
                if let Some(outcome) = &file.inspection.fix {
                    append_findings(&mut output, &file.path, &outcome.remaining);
                }
            }
            append_repository(&mut output);
        }
        return output;
    }

    let findings = repository.len()
        + files
            .iter()
            .map(|file| file.inspection.findings.len())
            .sum::<usize>();
    for file in files {
        append_findings(&mut output, &file.path, &file.inspection.findings);
    }
    append_repository(&mut output);
    output.push_str(&format!(
        "Checked {} workflow file(s); {findings} finding(s).\n",
        files.len()
    ));
    output
}

fn append_findings(
    output: &mut String,
    path: &std::path::Path,
    findings: &[crate::rules::Finding],
) {
    for finding in findings {
        output.push_str(&format!(
            "{} [{}:{}] {}:{}: {}: {}{}\n",
            finding.rule.id,
            finding.rule.tag,
            finding.severity.as_str(),
            path.display(),
            finding.line,
            finding.location,
            finding.message,
            finding.rule.equivalents_suffix()
        ));
    }
}

pub fn rules() -> String {
    crate::rules::all()
        .map(|(rule, scope)| {
            format!(
                "{} {} {} {} {scope}\n",
                rule.id,
                rule.tag,
                rule.severity.as_str(),
                rule.persona.as_str()
            )
        })
        .collect()
}

/// The `--list-rules` catalog with the fields SARIF rule descriptors carry.
pub fn rules_json() -> String {
    let rules: Vec<Value> = crate::rules::all()
        .map(|(rule, scope)| {
            json!({
                "id": rule.id,
                "tag": rule.tag,
                "severity": rule.severity.as_str(),
                "persona": rule.persona.as_str(),
                "scope": scope,
                "summary": rule.summary,
                "equivalents": equivalents(rule),
            })
        })
        .collect();
    let report = json!({
        "version": 1,
        "tool": { "name": "crosgha", "version": env!("CARGO_PKG_VERSION") },
        "rules": rules,
    });
    format!("{report:#}\n")
}

/// Every reported finding with its path: what remains after `--fix`, or everything found.
fn reported<'a>(
    files: &'a [FileReport],
    repository: &'a [Located],
    fix: bool,
) -> Vec<(&'a Path, &'a Finding)> {
    let mut found = Vec::new();
    for file in files {
        let findings = match (&file.inspection.fix, fix) {
            (Some(outcome), true) => &outcome.remaining,
            (None, true) => &[][..],
            (_, false) => &file.inspection.findings,
        };
        found.extend(
            findings
                .iter()
                .map(|finding| (file.path.as_path(), finding)),
        );
    }
    found.extend(
        repository
            .iter()
            .map(|located| (located.path.as_path(), &located.finding)),
    );
    found
}

fn fixed(files: &[FileReport]) -> usize {
    files
        .iter()
        .filter_map(|file| file.inspection.fix.as_ref())
        .map(|outcome| outcome.fixed_count)
        .sum()
}

fn equivalents(rule: &Rule) -> Value {
    rule.equivalents
        .iter()
        .map(|(tool, id)| json!({ "tool": tool, "rule": id }))
        .collect()
}

/// A stable, versioned JSON report for scripts and the Action.
fn json(files: &[FileReport], repository: &[Located], errors: &[FileError], fix: bool) -> String {
    let findings: Vec<Value> = reported(files, repository, fix)
        .into_iter()
        .map(|(path, finding)| {
            json!({
                "rule": finding.rule.id,
                "tag": finding.rule.tag,
                "severity": finding.severity.as_str(),
                "path": path.display().to_string(),
                "line": finding.line,
                "column": finding.column,
                "end_line": finding.end_line,
                "end_column": finding.end_column,
                "location": finding.location,
                "message": finding.message,
                "equivalents": equivalents(finding.rule),
            })
        })
        .collect();
    let errors: Vec<Value> = errors
        .iter()
        .map(|error| json!({ "path": error.path.display().to_string(), "message": error.message }))
        .collect();
    let report = json!({
        "version": 1,
        "tool": { "name": "crosgha", "version": env!("CARGO_PKG_VERSION") },
        "summary": {
            "files": files.len(),
            "findings": findings.len(),
            "fixed": fixed(files),
            "errors": errors.len(),
        },
        "findings": findings,
        "errors": errors,
    });
    format!("{report:#}\n")
}

/// Taxonomy tags outside the security dimensions (maintainability and correctness).
const QUALITY_TAGS: &[&str] = &[
    "missing-timeout",
    "error-suppression",
    "missing-concurrency",
    "broad-trigger",
    "missing-fork-guard",
    "missing-matrix/test-diversity",
    "cache-misuse",
    "redundant/duplicated-steps",
    "long-job/complexity",
    "readability",
    "deprecated-command/feature",
    "syntax/schema-validity",
    "embedded-script-lint",
    "insecure-protocol",
];

/// SARIF 2.1.0 with the fields GitHub code scanning requires:
/// https://docs.github.com/en/code-security/reference/code-scanning/sarif-files/sarif-support
/// `partialFingerprints` is left to `upload-sarif`, which computes GitHub's line hash.
pub fn sarif(
    files: &[FileReport],
    repository: &[Located],
    errors: &[FileError],
    fix: bool,
) -> String {
    let catalog: Vec<&Rule> = rules::all().map(|(rule, _)| rule).collect();
    let descriptors: Vec<Value> = catalog
        .iter()
        .map(|rule| {
            let security = !QUALITY_TAGS.contains(&rule.tag);
            let precision = match rule.persona {
                crate::policy::Persona::Regular => "high",
                crate::policy::Persona::Pedantic => "medium",
                crate::policy::Persona::Auditor => "low",
            };
            let mut properties = json!({
                "tags": [if security { "security" } else { "maintainability" }, rule.tag],
                "precision": precision,
            });
            if security {
                // GitHub maps 7.0-8.9 to high, 4.0-6.9 to medium, and 0.1-3.9 to low.
                let score = match rule.severity {
                    crate::policy::Severity::Error => "8.0",
                    crate::policy::Severity::Warning => "5.0",
                    crate::policy::Severity::Note => "2.0",
                };
                properties["security-severity"] = json!(score);
            } else {
                let problem = match rule.severity {
                    crate::policy::Severity::Error => "error",
                    crate::policy::Severity::Warning => "warning",
                    crate::policy::Severity::Note => "recommendation",
                };
                properties["problem.severity"] = json!(problem);
            }
            let help = format!(
                "{} Taxonomy tag: `{}`.{}",
                rule.summary,
                rule.tag,
                rule.equivalents_suffix()
            );
            json!({
                "id": rule.id,
                "name": rule.id,
                "shortDescription": { "text": rule.summary },
                "fullDescription": { "text": rule.summary },
                "help": { "text": help },
                "defaultConfiguration": { "level": rule.severity.as_str() },
                "properties": properties,
            })
        })
        .collect();
    let results: Vec<Value> = reported(files, repository, fix)
        .into_iter()
        .map(|(path, finding)| {
            let index = catalog
                .iter()
                .position(|rule| rule.id == finding.rule.id)
                .unwrap_or_default();
            json!({
                "ruleId": finding.rule.id,
                "ruleIndex": index,
                "level": finding.severity.as_str(),
                "message": { "text": format!("{}: {}", finding.location, finding.message) },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": artifact_uri(path) },
                        "region": {
                            "startLine": finding.line,
                            "startColumn": finding.column,
                            "endLine": finding.end_line,
                            "endColumn": finding.end_column,
                        },
                    },
                }],
            })
        })
        .collect();
    // Files that failed to parse are tool execution notifications, not results.
    let notifications: Vec<Value> = errors
        .iter()
        .map(|error| {
            json!({
                "level": "error",
                "message": { "text": error.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": artifact_uri(&error.path) },
                    },
                }],
            })
        })
        .collect();
    let report = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "crosgha",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": env!("CARGO_PKG_REPOSITORY"),
                    "rules": descriptors,
                },
            },
            "invocations": [{
                "executionSuccessful": errors.is_empty(),
                "toolExecutionNotifications": notifications,
            }],
            "results": results,
        }],
    });
    format!("{report:#}\n")
}

/// `::error`/`::warning`/`::notice` workflow commands, escaped like actions/toolkit does:
/// https://github.com/actions/toolkit/blob/main/packages/core/src/command.ts
fn github(files: &[FileReport], repository: &[Located], fix: bool) -> String {
    let data = |text: &str| {
        text.replace('%', "%25")
            .replace('\r', "%0D")
            .replace('\n', "%0A")
    };
    let property = |text: &str| data(text).replace(':', "%3A").replace(',', "%2C");
    reported(files, repository, fix)
        .into_iter()
        .map(|(path, finding)| {
            let command = match finding.severity {
                crate::policy::Severity::Error => "error",
                crate::policy::Severity::Warning => "warning",
                crate::policy::Severity::Note => "notice",
            };
            format!(
                "::{command} file={},line={},endLine={},col={},endColumn={},title={}::{}\n",
                property(&artifact_uri(path)),
                finding.line,
                finding.end_line,
                finding.column,
                finding.end_column,
                property(finding.rule.id),
                data(&format!(
                    "{}: {}{}",
                    finding.location,
                    finding.message,
                    finding.rule.equivalents_suffix()
                )),
            )
        })
        .collect()
}

/// `path:line:col: severity rule: message help: summary`, one line each, like oxlint's agent
/// format; the rule summary is the help because it states what the rule requires.
fn agent(files: &[FileReport], repository: &[Located], fix: bool) -> String {
    reported(files, repository, fix)
        .into_iter()
        .map(|(path, finding)| {
            let line = format!(
                "{}:{}:{}: {} {}: {}: {}{} help: {}",
                relative(path),
                finding.line,
                finding.column,
                finding.severity.as_str(),
                finding.rule.id,
                finding.location,
                finding.message,
                finding.rule.equivalents_suffix(),
                finding.rule.summary
            );
            line.replace(['\r', '\n'], " ") + "\n"
        })
        .collect()
}

/// A path relative to the working directory, which is the repository root in CI.
fn relative(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .trim_start_matches("./")
        .to_owned()
}

fn artifact_uri(path: &Path) -> String {
    relative(path).replace('%', "%25").replace(' ', "%20")
}
