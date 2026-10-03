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
    workflow::BOM,
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

/// `fix` is whether `--fix` was requested; each file's `unwritten` says whether its fixes reached
/// the disk.
pub fn render(
    format: Format,
    files: &[FileReport],
    repository: &[Located],
    errors: &[FileError],
    fix: bool,
) -> String {
    match format {
        Format::Text => text(files, repository, errors, fix),
        Format::Json => json(files, repository, errors),
        Format::Sarif => sarif(files, repository, errors),
        Format::Github => github(files, repository),
        Format::Agent => agent(files, repository),
    }
}

pub struct FileReport {
    pub path: PathBuf,
    pub inspection: Inspection,
    /// Why the verified fixes of this file were not written.
    pub unwritten: Option<String>,
}

impl FileReport {
    /// The source and findings to report: what remains in the written fix, or the original.
    pub fn shown(&self) -> (&str, &[Finding]) {
        match (&self.inspection.fix, &self.unwritten) {
            (Some(outcome), None) => (&outcome.source, &outcome.remaining),
            _ => (&self.inspection.source, &self.inspection.findings),
        }
    }

    /// Fixes written to the file.
    pub fn fixed(&self) -> usize {
        match (&self.inspection.fix, &self.unwritten) {
            (Some(outcome), None) => outcome.applied(),
            _ => 0,
        }
    }
}

/// What kept the run from checking or fixing everything, which makes it exit 2: a workflow that
/// could not be read, parsed, or written, or (`path` is `None`) a GitHub API failure whose online
/// checks were skipped. The text formats print it on stderr as it happens; JSON and SARIF carry
/// it so CI consumers see the failure.
pub struct FileError {
    pub path: Option<PathBuf>,
    pub message: String,
}

/// rustc-style diagnostics with the offending source line, then a summary. The styles are
/// ANSI codes that `anstream` strips unless stdout is a color terminal.
fn text(files: &[FileReport], repository: &[Located], errors: &[FileError], fix: bool) -> String {
    let mut output = String::new();
    let sources = repository_sources(repository);
    let mut shown = Vec::new();
    for (path, source, finding) in reported(files, repository, &sources) {
        diagnostic(&mut output, path, source, finding);
        shown.push(finding);
    }
    for (path, source, finding, _, reason) in attempts(files) {
        if let Some(reason) = reason {
            let _ = writeln!(
                output,
                "Not fixed: {}:{}:{} [{}]: {reason}",
                path.display(),
                finding.line,
                char_column(Some(source), finding.line, finding.column),
                finding.rule.id
            );
        }
    }
    let fixed: usize = files.iter().map(FileReport::fixed).sum();
    let failed = match errors.iter().filter(|error| error.path.is_some()).count() {
        0 => String::new(),
        failed => {
            let level = style(Severity::Error);
            format!(" ({level}{failed} failed{level:#})")
        }
    };
    if fixed > 0 {
        let changed = files.iter().filter(|file| file.fixed() > 0).count();
        output.push_str(&format!(
            "{SUCCESS}Fixed{SUCCESS:#} {} in {}{failed}.\n",
            count(fixed, "finding"),
            count(changed, "workflow file")
        ));
        if !shown.is_empty() {
            output.push_str(&format!(
                "Remaining: {}{}.\n",
                count(shown.len(), "finding"),
                breakdown(&shown)
            ));
        }
    } else if shown.is_empty() {
        let mark = match errors.is_empty() {
            true => format!("{SUCCESS}✓{SUCCESS:#} "),
            false => String::new(),
        };
        let checked = count(files.len(), "workflow file");
        output.push_str(&format!("{mark}Checked {checked}{failed}: no findings.\n"));
    } else {
        output.push_str(&format!(
            "Checked {}{failed}: {}{}.\n",
            count(files.len(), "workflow file"),
            count(shown.len(), "finding"),
            breakdown(&shown)
        ));
        let fixable = shown.iter().filter(|finding| finding.fix.is_some()).count();
        if fixable > 0 && !fix {
            output.push_str(&format!(
                "{} a fix: rerun with {BOLD}--fix{BOLD:#}.\n",
                match fixable {
                    1 => "1 finding has".to_owned(),
                    _ => format!("{fixable} findings have"),
                }
            ));
        }
    }
    if errors.iter().any(|error| error.path.is_none()) {
        let level = style(Severity::Error);
        output.push_str(&format!(
            "{level}The scan is incomplete:{level:#} online checks that needed a failed GitHub \
             API request did not run.\n"
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
    // Columns do not count a BOM, so line 1 is shown without it.
    let text = source
        .map(|source| source.strip_prefix(BOM).unwrap_or(source))
        .and_then(|source| source.lines().nth(finding.line.checked_sub(1)?));
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
                char_column(source, finding.line, finding.column)
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
                "configurable": rule.configurable,
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

/// Each workflow's reported findings with the source their lines point into.
fn workflows(files: &[FileReport]) -> impl Iterator<Item = (&Path, &str, &[Finding])> {
    files.iter().map(|file| {
        let (source, findings) = file.shown();
        (file.path.as_path(), source, findings)
    })
}

/// The content of each repository finding's file, `None` for a directory.
fn repository_sources(repository: &[Located]) -> Vec<Option<String>> {
    repository
        .iter()
        .map(|located| std::fs::read_to_string(&located.path).ok())
        .collect()
}

/// Every reported finding with its path and the source its lines point into: what remains
/// after `--fix`, or everything found. `sources` comes from `repository_sources`.
fn reported<'a>(
    files: &'a [FileReport],
    repository: &'a [Located],
    sources: &'a [Option<String>],
) -> Vec<(&'a Path, Option<&'a str>, &'a Finding)> {
    workflows(files)
        .flat_map(|(path, source, findings)| {
            findings
                .iter()
                .map(move |finding| (path, Some(source), finding))
        })
        .chain(
            repository.iter().zip(sources).map(|(located, source)| {
                (located.path.as_path(), source.as_deref(), &located.finding)
            }),
        )
        .collect()
}

/// Every proposed fix with its finding, its status, and why it is not in the file. A fix that
/// is not in the file points at its finding among the reported ones, whose lines an applied fix
/// above it may have moved; an applied fix points at the original source.
fn attempts(
    files: &[FileReport],
) -> impl Iterator<Item = (&Path, &str, &Finding, &'static str, Option<&str>)> {
    files.iter().flat_map(|file| {
        let (source, shown) = file.shown();
        file.inspection
            .fix
            .iter()
            .flat_map(|outcome| &outcome.attempts)
            .map(move |attempt| {
                let (status, reason) = match (&attempt.rejected, &file.unwritten) {
                    (Some(reason), _) => ("rejected", Some(reason.as_str())),
                    (None, Some(reason)) => ("not-written", Some(reason.as_str())),
                    (None, None) => ("applied", None),
                };
                let original = &file.inspection.findings[attempt.finding];
                let (source, finding) = shown
                    .iter()
                    .filter(|_| reason.is_some())
                    .find(|shown| {
                        shown.rule.id == original.rule.id && shown.location == original.location
                    })
                    .map_or((file.inspection.source.as_str(), original), |shown| {
                        (source, shown)
                    });
                (file.path.as_path(), source, finding, status, reason)
            })
    })
}

fn equivalents(rule: &Rule) -> Value {
    rule.equivalents
        .iter()
        .map(|(tool, id)| json!({ "tool": tool, "rule": id }))
        .collect()
}

/// A stable, versioned JSON report for scripts and the Action.
fn json(files: &[FileReport], repository: &[Located], errors: &[FileError]) -> String {
    let sources = repository_sources(repository);
    let findings: Vec<Value> = reported(files, repository, &sources)
        .into_iter()
        .map(|(path, source, finding)| {
            json!({
                "rule": finding.rule.id,
                "tag": finding.rule.tag,
                "severity": finding.severity.as_str(),
                "path": path.display().to_string(),
                "line": finding.line,
                "column": char_column(source, finding.line, finding.column),
                "end_line": finding.end_line,
                "end_column": char_column(source, finding.end_line, finding.end_column),
                "location": finding.location,
                "message": finding.message,
                "equivalents": equivalents(finding.rule),
            })
        })
        .collect();
    let fixes: Vec<Value> = attempts(files)
        .map(|(path, source, finding, status, reason)| {
            json!({
                "rule": finding.rule.id,
                "path": path.display().to_string(),
                "line": finding.line,
                "column": char_column(Some(source), finding.line, finding.column),
                "location": finding.location,
                "status": status,
                "reason": reason,
            })
        })
        .collect();
    let errors: Vec<Value> = errors
        .iter()
        .map(|error| {
            let path = error.path.as_ref().map(|path| path.display().to_string());
            json!({ "path": path, "message": error.message })
        })
        .collect();
    let report = json!({
        "version": 1,
        "tool": { "name": "crosgha", "version": env!("CARGO_PKG_VERSION") },
        "summary": {
            "files": files.len(),
            "findings": findings.len(),
            "fixed": files.iter().map(FileReport::fixed).sum::<usize>(),
            "errors": errors.len(),
        },
        "findings": findings,
        "fixes": fixes,
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
pub fn sarif(files: &[FileReport], repository: &[Located], errors: &[FileError]) -> String {
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
    let sources = repository_sources(repository);
    let results: Vec<Value> = reported(files, repository, &sources)
        .into_iter()
        .map(|(path, source, finding)| {
            let index = catalog
                .iter()
                .position(|rule| rule.id == finding.rule.id)
                .unwrap_or_default();
            // A repository-wide finding names its directory in the message. GitHub needs a file
            // location anyway ("At least one location is required for code scanning to display
            // a result", per the page above), so it points at line 1 of the first scanned
            // workflow of that `.github` directory, in path order.
            let (message, locations) = match path.is_dir() {
                true => (
                    format!(
                        "{}: {}: {}",
                        relative(path),
                        finding.location,
                        finding.message
                    ),
                    match first_workflow(path, files) {
                        Some((workflow, source)) => {
                            let line = source.strip_prefix(BOM).unwrap_or(source).lines().next();
                            json!([{
                                "physicalLocation": {
                                    "artifactLocation": { "uri": artifact_uri(workflow) },
                                    "region": {
                                        "startLine": 1,
                                        "startColumn": 1,
                                        "endLine": 1,
                                        "endColumn":
                                            line.unwrap_or_default().encode_utf16().count() + 1,
                                    },
                                },
                            }])
                        }
                        None => json!([]),
                    },
                ),
                false => (
                    format!("{}: {}", finding.location, finding.message),
                    json!([{
                        "physicalLocation": {
                            "artifactLocation": { "uri": artifact_uri(path) },
                            "region": {
                                "startLine": finding.line,
                                "startColumn": utf16_column(source, finding.line, finding.column),
                                "endLine": finding.end_line,
                                "endColumn":
                                    utf16_column(source, finding.end_line, finding.end_column),
                            },
                        },
                    }]),
                ),
            };
            json!({
                "ruleId": finding.rule.id,
                "ruleIndex": index,
                "level": finding.severity.as_str(),
                "message": { "text": message },
                "locations": locations,
            })
        })
        .collect();
    // Run failures are tool execution notifications, not results.
    let notifications: Vec<Value> = errors
        .iter()
        .map(|error| {
            let mut notification = json!({
                "level": "error",
                "message": { "text": error.message },
            });
            if let Some(path) = &error.path {
                notification["locations"] = json!([{
                    "physicalLocation": { "artifactLocation": { "uri": artifact_uri(path) } },
                }]);
            }
            notification
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
            "columnKind": "utf16CodeUnits",
            "invocations": [{
                "executionSuccessful": errors.is_empty(),
                "toolExecutionNotifications": notifications,
            }],
            "results": results,
        }],
    });
    format!("{report:#}\n")
}

/// The first scanned workflow inside `directory`, with its source.
fn first_workflow<'a>(directory: &Path, files: &'a [FileReport]) -> Option<(&'a Path, &'a str)> {
    let directory = crate::filesystem::identity(directory);
    workflows(files)
        .find(|(path, _, _)| crate::filesystem::identity(path).starts_with(&directory))
        .map(|(path, source, _)| (path, source))
}

/// `::error`/`::warning`/`::notice` workflow commands, escaped like actions/toolkit does:
/// https://github.com/actions/toolkit/blob/main/packages/core/src/command.ts
fn github(files: &[FileReport], repository: &[Located]) -> String {
    let data = |text: &str| {
        text.replace('%', "%25")
            .replace('\r', "%0D")
            .replace('\n', "%0A")
    };
    let property = |text: &str| data(text).replace(':', "%3A").replace(',', "%2C");
    let sources = repository_sources(repository);
    reported(files, repository, &sources)
        .into_iter()
        .map(|(path, source, finding)| {
            let command = match finding.severity {
                crate::policy::Severity::Error => "error",
                crate::policy::Severity::Warning => "warning",
                crate::policy::Severity::Note => "notice",
            };
            let message = format!(
                "{}: {}{}",
                finding.location,
                finding.message,
                finding.rule.equivalents_suffix()
            );
            // A repository-wide finding points at the workflow its SARIF result uses, since a
            // directory is not a file; its message names the directory instead.
            if path.is_dir() {
                let file = first_workflow(path, files)
                    .map(|(workflow, _)| {
                        format!("file={},line=1,endLine=1,", property(&relative(workflow)))
                    })
                    .unwrap_or_default();
                return format!(
                    "::{command} {file}title={}::{}\n",
                    property(finding.rule.id),
                    data(&format!("{}: {message}", relative(path))),
                );
            }
            // actions/toolkit: columns "cannot be sent when startLine and endLine are different
            // values".
            let columns = match finding.line == finding.end_line {
                true => format!(
                    ",col={},endColumn={}",
                    char_column(source, finding.line, finding.column),
                    char_column(source, finding.end_line, finding.end_column)
                ),
                false => String::new(),
            };
            format!(
                "::{command} file={},line={},endLine={}{columns},title={}::{}\n",
                property(&relative(path)),
                finding.line,
                finding.end_line,
                property(finding.rule.id),
                data(&message),
            )
        })
        .collect()
}

/// `path:line:col: severity rule: message help: summary`, one line each, like oxlint's agent
/// format; the rule summary is the help because it states what the rule requires.
fn agent(files: &[FileReport], repository: &[Located]) -> String {
    let sources = repository_sources(repository);
    reported(files, repository, &sources)
        .into_iter()
        .map(|(path, source, finding)| {
            let line = format!(
                "{}:{}:{}: {} {}: {}: {}{} help: {}",
                relative(path),
                finding.line,
                char_column(source, finding.line, finding.column),
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

/// A URI reference for `path` with every byte outside RFC 3986's unreserved characters and `/`
/// percent-encoded, so no character in a file name is read as URI syntax. A path outside the
/// working directory stays absolute and becomes a `file:` URI (RFC 8089).
fn artifact_uri(path: &Path) -> String {
    let path = relative(path);
    let mut uri = match path.starts_with('/') {
        true => "file://".to_owned(),
        false => String::new(),
    };
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(char::from(byte));
            }
            _ => {
                let _ = write!(uri, "%{byte:02X}");
            }
        }
    }
    uri
}

/// SARIF's `columnKind` is `utf16CodeUnits`.
fn utf16_column(source: Option<&str>, line: usize, column: usize) -> usize {
    recount(source, line, column, |text| text.encode_utf16().count())
}

/// The other formats count characters, as GitHub's annotations do.
fn char_column(source: Option<&str>, line: usize, column: usize) -> usize {
    recount(source, line, column, |text| text.chars().count())
}

/// Finding columns count UTF-8 bytes; `unit` measures the text before one in another unit.
fn recount(source: Option<&str>, line: usize, column: usize, unit: fn(&str) -> usize) -> usize {
    source
        .map(|source| source.strip_prefix(BOM).unwrap_or(source))
        .and_then(|source| source.lines().nth(line.checked_sub(1)?))
        .and_then(|text| text.get(..column.checked_sub(1)?))
        .map_or(column, |before| unit(before) + 1)
}
