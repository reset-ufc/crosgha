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

/// Per directory, `.github/` comes before the directory itself, as zizmor's discovery orders
/// `.github/zizmor.yml` before `zizmor.yml` (https://docs.zizmor.sh/configuration/#discovery);
/// crosgha's own file wins over zizmor's.
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
    /// repository root, its `.github`, its `.github/workflows`, and a workflow in it all start at
    /// the root, so the config never depends on how inputs are spelled or ordered, and a workflow
    /// named `zizmor.yml` is never read as configuration.
    pub fn discover(input: &Path) -> Result<Option<(PathBuf, Self)>, String> {
        // A symlinked workflow belongs to the directory that holds the link.
        let directory = if input.is_dir() {
            input
        } else {
            input
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
        };
        let directory = fs::canonicalize(directory)
            .map_err(|error| format!("cannot resolve `{}`: {error}", input.display()))?;
        let mut current = if directory.ends_with(".github/workflows") {
            directory.parent().and_then(Path::parent)
        } else if directory.ends_with(".github") {
            directory.parent()
        } else {
            Some(directory.as_path())
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
        // zizmor rejects unknown keys at every level of its schema
        // (https://docs.zizmor.sh/configuration/#json-schema), so a typo cannot leave a rule
        // silently enabled. `null` is read as an absent key.
        let known = |value: &Value, at: &str, keys: &[&str]| -> Result<(), String> {
            let unknown = value
                .as_mapping()
                .into_iter()
                .flatten()
                .map(|(key, _)| key.as_str().unwrap_or("?"))
                .find(|key| !keys.contains(key));
            match unknown {
                Some(key) => Err(describe(format!(
                    "unknown key `{at}{key}`; expected {}",
                    keys.join(", ")
                ))),
                None => Ok(()),
            }
        };
        let present = |value: &Value, key: &str| value.get(key).cloned().filter(|v| !v.is_null());
        match &value {
            Value::Null => return Ok(config),
            Value::Mapping(_) => known(&value, "", &["rules"])?,
            _ => {
                return Err(describe(
                    "the config must be a mapping with a `rules` key".to_owned(),
                ));
            }
        }
        let rules = match present(&value, "rules") {
            None => return Ok(config),
            Some(Value::Mapping(rules)) => rules,
            Some(_) => return Err(describe("`rules` must be a mapping of rule IDs".to_owned())),
        };
        for (id, settings) in &rules {
            let Some(id) = id.as_str() else {
                return Err(describe("`rules` keys must be rule IDs".to_owned()));
            };
            if !(settings.is_null() || settings.is_mapping()) {
                return Err(describe(format!("`rules.{id}` must be a mapping")));
            }
            known(
                settings,
                &format!("rules.{id}."),
                &["disable", "ignore", "remap", "config"],
            )?;
            let disable = match present(settings, "disable") {
                None => false,
                Some(Value::Bool(disable)) => disable,
                Some(_) => {
                    return Err(describe(format!(
                        "`rules.{id}.disable` must be true or false"
                    )));
                }
            };
            let ignore = match present(settings, "ignore") {
                None => Vec::new(),
                Some(Value::Sequence(entries)) => entries
                    .iter()
                    .map(|entry| {
                        let entry = entry.as_str().ok_or_else(|| {
                            describe(format!("`rules.{id}.ignore` entries must be strings"))
                        })?;
                        ignore_entry(entry).ok_or_else(|| {
                            describe(format!(
                                "`rules.{id}.ignore` entry `{entry}` is not `file`, \
                                 `file:line`, or `file:line:column`"
                            ))
                        })
                    })
                    .collect::<Result<_, _>>()?,
                Some(_) => return Err(describe(format!("`rules.{id}.ignore` must be a list"))),
            };
            let remapped = match present(settings, "remap") {
                None => None,
                Some(remap @ Value::Mapping(_)) => {
                    known(&remap, &format!("rules.{id}.remap."), &["severity"])?;
                    present(&remap, "severity")
                }
                Some(_) => return Err(describe(format!("`rules.{id}.remap` must be a mapping"))),
            };
            let severity = match remapped {
                None => None,
                Some(name) => Some(name.as_str().and_then(severity).ok_or_else(|| {
                    describe(format!(
                        "`rules.{id}.remap.severity` must be informational, low, medium, or high"
                    ))
                })?),
            };
            let options = match present(settings, "config") {
                None => None,
                Some(options @ Value::Mapping(_)) => Some(options),
                Some(_) => return Err(describe(format!("`rules.{id}.config` must be a mapping"))),
            };
            config.rules.push((
                id.to_owned(),
                RuleConfig {
                    disable,
                    ignore,
                    severity,
                    options,
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
