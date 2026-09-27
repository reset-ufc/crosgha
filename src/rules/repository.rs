//! Rules that read repository files next to the workflows, not a single workflow.

use std::{
    fs,
    path::{Path, PathBuf},
};

use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// zizmor `dependabot-cooldown` and Dependabot's `cooldown` option:
// https://docs.github.com/en/code-security/dependabot/working-with-dependabot/dependabot-options-reference#cooldown-
pub const COOLDOWN: Rule = Rule {
    id: "missing-release-cooldown",
    tag: "missing-release-cooldown",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    aliases: &["dependabot-cooldown"],
};

// Fares et al. (arXiv 2601.14455) FGB-05 "Hardening Gap Weakness" and the StepSecurity
// secure-repo checks (Dependabot, CodeQL, dependency review, OpenSSF Scorecard).
pub const TOOLING: Rule = Rule {
    id: "missing-security-tooling",
    tag: "missing-security-tooling",
    severity: Severity::Note,
    persona: Persona::Auditor,
    aliases: &[],
};

pub struct Located {
    pub path: PathBuf,
    pub finding: Finding,
}

pub fn cooldown(github: &Path, honor_ignores: bool) -> Result<Vec<Located>, String> {
    let mut found = Vec::new();
    for name in ["dependabot.yml", "dependabot.yaml"] {
        let path = github.join(name);
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let config =
            Workflow::parse_yaml(source).map_err(|error| format!("{}: {error}", path.display()))?;
        let Some(updates) = config.value.get("updates").and_then(Value::as_sequence) else {
            continue;
        };
        for (index, update) in updates.iter().enumerate() {
            if update.get("cooldown").is_some() {
                continue;
            }
            let ecosystem = update
                .get("package-ecosystem")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let finding = common::finding(
                &config,
                &COOLDOWN,
                Route::default().with_key("updates").with_key(index),
                format!("update `{ecosystem}`"),
                "updates are adopted without a minimum release age",
            )?;
            if !honor_ignores || !super::ignored(&config, &finding) {
                found.push(Located {
                    path: path.clone(),
                    finding,
                });
            }
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

pub fn tooling(github: &Path, _honor_ignores: bool) -> Result<Vec<Located>, String> {
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
                route: Route::default(),
                fix: None,
            },
        })
        .collect())
}
