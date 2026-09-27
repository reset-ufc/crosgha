//! Rules that read repository files next to the workflows, not a single workflow.

use std::{
    fs,
    path::{Path, PathBuf},
};

use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Options, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

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
};

pub struct Located {
    pub path: PathBuf,
    pub finding: Finding,
}

/// zizmor's default for `rules.dependabot-cooldown.config.days`.
const DEFAULT_COOLDOWN_DAYS: u64 = 7;

/// Dependabot waits this long when `cooldown` or its `default-days` is absent:
/// https://docs.github.com/en/code-security/dependabot/working-with-dependabot/dependabot-options-reference#cooldown-
const DEPENDABOT_DEFAULT_DAYS: u64 = 3;

pub fn cooldown(
    github: &Path,
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
    let Ok(source) = fs::read_to_string(path) else {
        return Ok(found);
    };
    let config =
        Workflow::parse_yaml(source).map_err(|error| format!("{}: {error}", path.display()))?;
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
    _honor_ignores: bool,
    _options: Options,
) -> Result<Vec<Located>, String> {
    let root = github.parent().unwrap_or(github);
    let mut workflows = String::new();
    if let Ok(entries) = fs::read_dir(github.join("workflows")) {
        for entry in entries.flatten() {
            workflows.push_str(
                &fs::read_to_string(entry.path())
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
            );
        }
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
            missing.push(format!("a {name} workflow"));
        }
    }
    Ok(missing
        .into_iter()
        .map(|what| Located {
            path: github.to_path_buf(),
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
                concept: None,
            },
        })
        .collect())
}

// GitHub retired these JavaScript action runtimes; runners now run such actions on Node 24:
// https://github.blog/changelog/2023-05-04-github-actions-all-actions-will-run-on-node16-instead-of-node12/
// https://github.blog/changelog/2024-09-25-end-of-life-for-actions-node16/
// https://github.blog/changelog/2026-09-23-node-20-is-no-longer-available-in-github-actions/
pub const RUNTIME: Rule = Rule {
    id: "retired-runtime",
    summary: "JavaScript actions must not declare a retired Node.js runtime.",
    tag: "deprecated-command/feature",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[],
    configurable: false,
};

const RETIRED_RUNTIMES: &[&str] = &["node12", "node16", "node20"];

/// Checks the action this repository publishes (`action.yml` at the root) and every local action
/// its workflows use. Unparseable files are skipped: the workflow scan already reports them.
pub fn runtime(
    github: &Path,
    honor_ignores: bool,
    _options: Options,
) -> Result<Vec<Located>, String> {
    let root = github.parent().unwrap_or(github);
    let mut actions: Vec<PathBuf> = vec![root.to_path_buf()];
    for entry in fs::read_dir(github.join("workflows"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let Some(workflow) = fs::read_to_string(entry.path())
            .ok()
            .and_then(|source| Workflow::parse_yaml(source).ok())
        else {
            continue;
        };
        for (_, _, uses) in common::all_uses(&workflow) {
            if let Some(path) = uses.strip_prefix("./") {
                actions.push(root.join(path));
            }
        }
    }
    actions.sort();
    actions.dedup();

    let mut found = Vec::new();
    for directory in actions {
        let Some((path, action)) = ["action.yml", "action.yaml"].iter().find_map(|name| {
            let path = directory.join(name);
            let action = Workflow::parse_yaml(fs::read_to_string(&path).ok()?).ok()?;
            Some((path, action))
        }) else {
            continue;
        };
        let Some(using) = action
            .value
            .get("runs")
            .and_then(|runs| runs.get("using"))
            .and_then(Value::as_str)
            .filter(|using| RETIRED_RUNTIMES.contains(using))
        else {
            continue;
        };
        let finding = common::finding(
            &action,
            &RUNTIME,
            Route::default().with_key("runs").with_key("using"),
            "action",
            format!(
                "`{using}` is retired; runners run this action on Node 24, so declare `node24`"
            ),
        )?;
        if !honor_ignores || !super::ignored(&action, &finding) {
            found.push(Located { path, finding });
        }
    }
    Ok(found)
}
