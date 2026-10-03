//! `crosgha.yml`, or an existing `zizmor.yml`, with zizmor's schema and discovery rules so a
//! project can switch tools without rewriting its configuration:
//! https://docs.zizmor.sh/configuration/

use std::{
    fs,
    path::{Path, PathBuf},
};

use yaml_serde::Value;

use crate::{
    policy::Severity,
    rules::{Finding, Rule},
};

#[derive(Default)]
pub struct Config {
    rules: Vec<(String, RuleConfig)>,
}

#[derive(Default)]
struct RuleConfig {
    disable: bool,
    /// `file.yml`, `file.yml:line`, or `file.yml:line:column` (the column is not checked).
    ignore: Vec<(String, Option<usize>)>,
    severity: Option<Severity>,
    /// `rules.<id>.config`, read by the rule itself.
    options: Option<Value>,
}

const NAMES: &[&str] = &[
    ".github/crosgha.yml",
    ".github/crosgha.yaml",
    "crosgha.yml",
    "crosgha.yaml",
    ".github/zizmor.yml",
    ".github/zizmor.yaml",
    "zizmor.yml",
    "zizmor.yaml",
];

/// zizmor severities mapped onto SARIF levels.
pub fn severity(name: &str) -> Option<Severity> {
    match name {
        "informational" | "low" => Some(Severity::Note),
        "medium" => Some(Severity::Warning),
        "high" => Some(Severity::Error),
        _ => None,
    }
}

/// A malformed position is `None`, never "any line": that would silence the whole file.
fn ignore_entry(entry: &str) -> Option<(String, Option<usize>)> {
    let mut parts = entry.split(':');
    let file = parts.next()?.to_owned();
    let line = parts.next().map(str::parse).transpose().ok()?;
    if let Some(column) = parts.next() {
        column.parse::<usize>().ok()?;
    }
    parts.next().is_none().then_some((file, line))
}

impl Config {
    /// The first config file found walking up from `input`, stopping at the Git root. A
    /// `.github/workflows` directory, or a file in it, starts two levels up so a workflow named
    /// `zizmor.yml` is never read as configuration.
    pub fn discover(input: &Path) -> Result<Option<(PathBuf, Self)>, String> {
        let input = fs::canonicalize(input)
            .map_err(|error| format!("cannot resolve `{}`: {error}", input.display()))?;
        let directory = if input.is_dir() {
            Some(input.as_path())
        } else {
            input.parent()
        };
        let mut current = match directory {
            Some(workflows) if workflows.ends_with(".github/workflows") => {
                workflows.parent().and_then(Path::parent)
            }
            other => other,
        };
        while let Some(directory) = current {
            if let Some(path) = NAMES
                .iter()
                .map(|name| directory.join(name))
                .find(|path| path.is_file())
            {
                let config = Self::load(&path)?;
                return Ok(Some((path, config)));
            }
            if directory.join(".git").exists() {
                break;
            }
            current = directory.parent();
        }
        Ok(None)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let describe = |error: String| format!("config `{}`: {error}", path.display());
        let source = fs::read_to_string(path).map_err(|error| describe(error.to_string()))?;
        let value: Value =
            yaml_serde::from_str(&source).map_err(|error| describe(error.to_string()))?;
        let mut config = Self::default();
        let Some(rules) = value.get("rules").and_then(Value::as_mapping) else {
            return Ok(config);
        };
        for (id, settings) in rules {
            let Some(id) = id.as_str() else { continue };
            let ignore = settings
                .get("ignore")
                .and_then(Value::as_sequence)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|entry| {
                    ignore_entry(entry).ok_or_else(|| {
                        describe(format!(
                            "`rules.{id}.ignore` entry `{entry}` is not `file`, `file:line`, or \
                             `file:line:column`"
                        ))
                    })
                })
                .collect::<Result<_, _>>()?;
            let remapped = settings
                .get("remap")
                .and_then(|remap| remap.get("severity"))
                .and_then(Value::as_str);
            config.rules.push((
                id.to_owned(),
                RuleConfig {
                    disable: settings.get("disable").and_then(Value::as_bool) == Some(true),
                    ignore,
                    severity: remapped.and_then(severity),
                    options: settings.get("config").cloned(),
                },
            ));
        }
        Ok(config)
    }

    fn entries(&self, rule: &'static Rule) -> impl Iterator<Item = &RuleConfig> {
        self.rules
            .iter()
            .filter(|(id, _)| rule.answers_to(id))
            .map(|(_, settings)| settings)
    }

    /// Settings keyed by an ID that names this finding (see `Finding::answers_to`).
    fn for_finding<'a>(&'a self, finding: &'a Finding) -> impl Iterator<Item = &'a RuleConfig> {
        self.rules
            .iter()
            .filter(|(id, _)| finding.answers_to(id))
            .map(|(_, settings)| settings)
    }

    /// The whole rule is disabled, so it need not run.
    pub fn disabled(&self, rule: &'static Rule) -> bool {
        self.rules
            .iter()
            .any(|(id, settings)| settings.disable && rule.whole(id))
    }

    /// A key for part of a rule disables the findings of that part.
    pub fn drops(&self, finding: &Finding) -> bool {
        self.for_finding(finding).any(|settings| settings.disable)
    }

    pub fn ignores(&self, finding: &Finding, file: &str) -> bool {
        self.for_finding(finding).any(|settings| {
            settings
                .ignore
                .iter()
                .any(|(name, at)| name == file && at.is_none_or(|at| at == finding.line))
        })
    }

    pub fn options(&self, rule: &'static Rule) -> Option<&Value> {
        self.entries(rule)
            .find_map(|settings| settings.options.as_ref())
    }

    /// IDs that carry a `config` block, to warn about rules that cannot read one.
    pub fn configured_ids(&self) -> impl Iterator<Item = &str> {
        self.rules
            .iter()
            .filter(|(_, settings)| settings.options.is_some())
            .map(|(id, _)| id.as_str())
    }

    pub fn severity(&self, finding: &Finding) -> Option<Severity> {
        self.for_finding(finding)
            .find_map(|settings| settings.severity)
    }
}
