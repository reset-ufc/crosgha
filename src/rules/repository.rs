//! Rules that read repository files next to the workflows, not a single workflow.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Options, Rule, common};
use crate::filesystem;
use crate::policy::{Persona, Severity};
use crate::workflow::Workflow;

// zizmor `dependabot-cooldown` and Dependabot's `cooldown` option:
// https://docs.github.com/en/code-security/dependabot/working-with-dependabot/dependabot-options-reference#cooldown-
pub const COOLDOWN: Rule = Rule {
    id: "missing-release-cooldown",
    summary: "Dependabot updates should wait for a minimum release age.",
    tag: "missing-release-cooldown",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[("zizmor", "dependabot-cooldown")],
    configurable: true,
    relational: false,
};

// Fares et al. (arXiv 2601.14455) FGB-05 "Hardening Gap Weakness" and the StepSecurity
// secure-repo checks (Dependabot, CodeQL, dependency review, OpenSSF Scorecard).
pub const TOOLING: Rule = Rule {
    id: "missing-security-tooling",
    summary: "Repositories should run dependency updates, CodeQL, dependency review, and OpenSSF Scorecard.",
    tag: "missing-security-tooling",
    severity: Severity::Note,
    persona: Persona::Auditor,
    equivalents: &[],
    configurable: false,
    relational: false,
};

pub struct Located {
    pub path: PathBuf,
    pub place: Place,
    pub finding: Finding,
}

/// What a repository finding's lines point into.
pub enum Place {
    /// The text of the file at `path`.
    File(String),
    /// The repository as a whole, named by its `.github` directory or the scanned input. A
    /// directory is not a file, so annotations and SARIF place the finding on `anchor`, a scanned
    /// workflow inside it, which the scan sets once the workflows are known.
    Repository { anchor: Option<PathBuf> },
}

/// The source of each file the scan already read, by `filesystem::identity` of its path, so
/// repository rules do not read it again.
pub type Scanned<'a> = HashMap<PathBuf, &'a str>;

/// Every entry of `github/workflows`, with the source the scan read from it. Entries the scan
/// skipped or failed to parse as a workflow come back as `None`.
fn workflow_files<'a>(github: &Path, scanned: &'a Scanned) -> Vec<(PathBuf, Option<&'a str>)> {
    let directory = github.join("workflows");
    // `filesystem::identity` of each entry, resolving their shared directory once.
    let resolved = fs::canonicalize(&directory).ok();
    fs::read_dir(&directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| {
            let path = entry.path();
            let key = match &resolved {
                Some(directory) => directory.join(entry.file_name()),
                None => path.clone(),
            };
            let source = scanned.get(&key).copied();
            (path, source)
        })
        .collect()
}

/// zizmor's default for `rules.dependabot-cooldown.config.days`.
const DEFAULT_COOLDOWN_DAYS: u64 = 7;

/// Dependabot waits this long when `cooldown` or its `default-days` is absent:
/// https://docs.github.com/en/code-security/dependabot/working-with-dependabot/dependabot-options-reference#cooldown-
const DEPENDABOT_DEFAULT_DAYS: u64 = 3;

pub fn cooldown(
    github: &Path,
    _scanned: &Scanned,
    honor_ignores: bool,
    options: Options,
) -> Result<Vec<Located>, String> {
    let mut found = Vec::new();
    for name in ["dependabot.yml", "dependabot.yaml"] {
        found.extend(cooldown_file(&github.join(name), honor_ignores, options)?);
    }
    Ok(found)
}

/// Checks one Dependabot config; a missing file has no findings.
pub fn cooldown_file(
    path: &Path,
    honor_ignores: bool,
    options: Options,
) -> Result<Vec<Located>, String> {
    let minimum = options
        .and_then(|options| options.get("days"))
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_COOLDOWN_DAYS);
    let mut found = Vec::new();
    let Ok(source) = filesystem::read(path) else {
        return Ok(found);
    };
    let config = Workflow::parse_yaml(source.text)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let Some(updates) = config.value.get("updates").and_then(Value::as_sequence) else {
        return Ok(found);
    };
    for (index, update) in updates.iter().enumerate() {
        let configured = update
            .get("cooldown")
            .and_then(|cooldown| cooldown.get("default-days"))
            .and_then(Value::as_u64);
        let days = configured.unwrap_or(DEPENDABOT_DEFAULT_DAYS);
        if days >= minimum {
            continue;
        }
        let source = if configured.is_some() {
            ""
        } else {
            " (Dependabot's default)"
        };
        let message =
            format!("cooldown of {days} day(s){source} is below the {minimum}-day minimum");
        let ecosystem = update
            .get("package-ecosystem")
            .and_then(Value::as_str)
            .unwrap_or("?");
        let finding = common::finding(
            &config,
            &COOLDOWN,
            Route::default().with_key("updates").with_key(index),
            format!("update `{ecosystem}`"),
            message,
        )?;
        if !honor_ignores || !super::ignored(&config, &finding) {
            found.push(Located {
                path: path.to_path_buf(),
                place: Place::File(config.document.source().to_owned()),
                finding,
            });
        }
    }
    Ok(found)
}

const UPDATE_CONFIGS: &[&str] = &[
    ".github/dependabot.yml",
    ".github/dependabot.yaml",
    ".github/renovate.json",
    ".github/renovate.json5",
    "renovate.json",
    "renovate.json5",
    ".renovaterc",
    ".renovaterc.json",
];

const SCANNERS: &[(&str, &str)] = &[
    ("CodeQL", "github/codeql-action/"),
    ("dependency review", "actions/dependency-review-action"),
    ("OpenSSF Scorecard", "ossf/scorecard-action"),
];

pub fn tooling(
    github: &Path,
    scanned: &Scanned,
    _honor_ignores: bool,
    _options: Options,
) -> Result<Vec<Located>, String> {
    let root = github.parent().unwrap_or(github);
    let mut workflows = String::new();
    for (path, source) in workflow_files(github, scanned) {
        let source = match source {
            Some(source) => source.to_ascii_lowercase(),
            None => filesystem::read(&path)
                .map(|source| source.text.to_ascii_lowercase())
                .unwrap_or_default(),
        };
        workflows.push_str(&source);
    }
    let mut missing = Vec::new();
    if !UPDATE_CONFIGS
        .iter()
        .any(|config| root.join(config).is_file())
    {
        missing.push("automated dependency updates (Dependabot or Renovate)".to_owned());
    }
    for (name, action) in SCANNERS {
        if !workflows.contains(action) {
            missing.push(format!("{name} workflow"));
        }
    }
    Ok(missing
        .into_iter()
        .map(|what| Located {
            path: github.to_path_buf(),
            place: Place::Repository { anchor: None },
            finding: Finding {
                rule: &TOOLING,
                severity: TOOLING.severity,
                message: format!("repository has no {what}"),
                location: "repository".to_owned(),
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 1,
                route: Route::default(),
                fix: None,
                concepts: &[],
            },
        })
        .collect())
}
