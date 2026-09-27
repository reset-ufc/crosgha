use std::{
    fmt::{Display, Write},
    path::{Path, PathBuf},
};

use anstyle::{AnsiColor, Style};
use clap::ValueEnum;

use serde_json::{Value, json};

use crate::{
    engine::Inspection,
    policy::Severity,
    rules::{self, Finding, Located, Rule},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Format {
    /// Diagnostics with source snippets for people; `plain` is zizmor's name for it.
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
        Format::Text => text(files, repository, errors, fix),
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

/// rustc-style diagnostics with the offending source line, then a summary. The styles are
/// ANSI codes that `anstream` strips unless stdout is a color terminal.
fn text(files: &[FileReport], repository: &[Located], errors: &[FileError], fix: bool) -> String {
    let mut output = String::new();
    let mut shown = Vec::new();
    for (path, source, findings) in workflows(files, fix) {
        for finding in findings {
            diagnostic(&mut output, path, Some(source), finding);
            shown.push(finding);
        }
    }
    for located in repository {
        let source = std::fs::read_to_string(&located.path).ok();
        diagnostic(
            &mut output,
            &located.path,
            source.as_deref(),
            &located.finding,
        );
        shown.push(&located.finding);
    }
    if fix {
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
            "{SUCCESS}Fixed{SUCCESS:#} {} in {}.\n",
            count(fixed(files), "finding"),
            count(changed, "workflow file")
        ));
        if !shown.is_empty() {
            output.push_str(&format!(
                "Remaining: {}{}.\n",
                count(shown.len(), "finding"),
                breakdown(&shown)
            ));
        }
        return output;
    }
    let mut checked = count(files.len(), "workflow file");
    if !errors.is_empty() {
        let level = style(Severity::Error);
        checked.push_str(&format!(" ({level}{} failed{level:#})", errors.len()));
    }
    if shown.is_empty() {
        let mark = match errors.is_empty() {
            true => format!("{SUCCESS}✓{SUCCESS:#} "),
            false => String::new(),
        };
        output.push_str(&format!("{mark}Checked {checked}: no findings.\n"));
        return output;
    }
    output.push_str(&format!(
        "Checked {checked}: {}{}.\n",
        count(shown.len(), "finding"),
        breakdown(&shown)
    ));
    let fixable = shown.iter().filter(|finding| finding.fix.is_some()).count();
    if fixable > 0 {
        output.push_str(&format!(
            "{} a fix: rerun with {BOLD}--fix{BOLD:#}.\n",
            match fixable {
                1 => "1 finding has".to_owned(),
                _ => format!("{fixable} findings have"),
            }
        ));
    }
    output
}

const BOLD: Style = Style::new().bold();
const GUTTER: Style = AnsiColor::Blue.on_default().bold();
const SUCCESS: Style = AnsiColor::Green.on_default().bold();

fn style(severity: Severity) -> Style {
    match severity {
        Severity::Error => AnsiColor::Red.on_default().bold(),
        Severity::Warning => AnsiColor::Yellow.on_default().bold(),
        Severity::Note => AnsiColor::Cyan.on_default().bold(),
    }
}

/// `3 findings`, `1 workflow file`.
fn count(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

/// ` (1 error, 2 notes)`, colored by severity, skipping severities without findings.
fn breakdown(findings: &[&Finding]) -> String {
    let parts: Vec<String> = [Severity::Error, Severity::Warning, Severity::Note]
        .into_iter()
        .filter_map(|severity| {
            let n = findings
                .iter()
                .filter(|finding| finding.severity == severity)
                .count();
            let level = style(severity);
            (n > 0).then(|| format!("{level}{}{level:#}", count(n, severity.as_str())))
        })
        .collect();
    format!(" ({})", parts.join(", "))
}

fn diagnostic(output: &mut String, path: &Path, source: Option<&str>, finding: &Finding) {
    let level = style(finding.severity);
    let _ = writeln!(
        output,
        "{level}{}[{}]{level:#}{BOLD}: {}{BOLD:#}",
        finding.severity.as_str(),
        finding.rule.id,
        finding.message
    );
    let text = source.and_then(|source| source.lines().nth(finding.line.checked_sub(1)?));
    let number = finding.line.to_string();
    let pad = " ".repeat(number.len());
    match (finding.line, text) {
        (0, _) => {
            let _ = writeln!(output, "{pad}{GUTTER}-->{GUTTER:#} {}", path.display());
        }
        _ => {
            let _ = writeln!(
                output,
                "{pad}{GUTTER}-->{GUTTER:#} {}:{}:{}",
                path.display(),
                finding.line,
                finding.column
            );
        }
    }
    if let Some(text) = text {
        // Columns are byte offsets; a span past the line's end (a multi-line node) stops there.
        let start = finding.column.saturating_sub(1).min(text.len());
        let end = match finding.end_line == finding.line {
            true => finding.end_column.saturating_sub(1).min(text.len()),
            false => text.trim_end().len(),
        };
        // ponytail: one space per char, so wide (CJK) characters misalign the carets; use
        // unicode-width if that shows up in real workflows.
        let indent: String = text
            .get(..start)
            .unwrap_or_default()
            .chars()
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .collect();
        let carets = "^".repeat(
            text.get(start..end)
                .map_or(0, |span| span.chars().count())
                .max(1),
        );
        let _ = writeln!(output, "{pad} {GUTTER}|{GUTTER:#}");
        let _ = writeln!(output, "{GUTTER}{number} |{GUTTER:#} {text}");
        let _ = writeln!(
            output,
            "{pad} {GUTTER}|{GUTTER:#} {indent}{level}{carets} {}{level:#}",
            finding.location
        );
        let _ = writeln!(output, "{pad} {GUTTER}|{GUTTER:#}");
    } else {
        let _ = writeln!(
            output,
            "{pad} {GUTTER}={GUTTER:#} {BOLD}at{BOLD:#}: {}",
            finding.location
        );
    }
    let _ = writeln!(
        output,
        "{pad} {GUTTER}={GUTTER:#} {BOLD}help{BOLD:#}: {}",
        finding.rule.summary
    );
    let equivalents = finding.rule.equivalents_suffix();
    let mut notes = Vec::new();
    if finding.rule.tag != finding.rule.id {
        notes.push(format!("taxonomy tag `{}`", finding.rule.tag));
    }
    if let Some(named) = equivalents
        .strip_prefix(" (")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        notes.push(format!("same as {named}"));
    }
    if !notes.is_empty() {
        let _ = writeln!(
            output,
            "{pad} {GUTTER}={GUTTER:#} {BOLD}note{BOLD:#}: {}",
            notes.join("; ")
        );
    }
    output.push('\n');
}

pub fn rules() -> String {
    let catalog: Vec<(&Rule, &str)> = crate::rules::all().collect();
    let width = |field: fn(&Rule) -> &str| {
        catalog
            .iter()
            .map(|(rule, _)| field(rule).len())
            .max()
            .unwrap_or_default()
    };
    let (id, tag) = (width(|rule| rule.id), width(|rule| rule.tag));
    catalog
        .iter()
        .map(|(rule, scope)| {
            let level = style(rule.severity);
            format!(
                "{BOLD}{:id$}{BOLD:#}  {:tag$}  {level}{:7}{level:#}  {:8}  {scope}\n",
                rule.id,
                rule.tag,
                rule.severity.as_str(),
                rule.persona.as_str()
            )
        })
        .collect()
}

/// `error: message` on stderr, styled like the report.
pub fn error(message: impl Display) {
    let level = style(Severity::Error);
    anstream::eprintln!("{level}error{level:#}: {message}");
}

pub fn warning(message: impl Display) {
    let level = style(Severity::Warning);
    anstream::eprintln!("{level}warning{level:#}: {message}");
}

pub fn note(message: impl Display) {
    let level = style(Severity::Note);
    anstream::eprintln!("{level}note{level:#}: {message}");
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

/// Each workflow's reported findings with the source their lines point into: what remains in the
/// fixed source after `--fix`, or everything found in the original.
fn workflows(files: &[FileReport], fix: bool) -> impl Iterator<Item = (&Path, &str, &[Finding])> {
    files
        .iter()
        .filter_map(move |file| match (&file.inspection.fix, fix) {
            (Some(outcome), true) => Some((
                file.path.as_path(),
                outcome.source.as_str(),
                &outcome.remaining[..],
            )),
            (None, true) => None,
            (_, false) => Some((
                file.path.as_path(),
                file.inspection.source.as_str(),
                &file.inspection.findings[..],
            )),
        })
}

/// Every reported finding with its path: what remains after `--fix`, or everything found.
fn reported<'a>(
    files: &'a [FileReport],
    repository: &'a [Located],
    fix: bool,
) -> Vec<(&'a Path, &'a Finding)> {
    workflows(files, fix)
        .flat_map(|(path, _, findings)| findings.iter().map(move |finding| (path, finding)))
        .chain(
            repository
                .iter()
                .map(|located| (located.path.as_path(), &located.finding)),
        )
        .collect()
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
