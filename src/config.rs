//! `crosgha.yml`, or an existing `zizmor.yml`, with zizmor's schema and discovery rules so a
//! project can switch tools without rewriting its configuration:
//! https://docs.zizmor.sh/configuration/

use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
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

// zizmor rejects unknown keys at every level of its schema
// (https://docs.zizmor.sh/configuration/#json-schema), so a typo cannot leave a rule silently
// enabled. `null` is read as an absent key.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, expecting = "a mapping with a `rules` key")]
struct File {
    rules: Option<Rules>,
}

/// Rule entries in file order, so the first key that names a rule wins.
struct Rules(Vec<(String, Option<Settings>)>);

impl<'de> Deserialize<'de> for Rules {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries;
        impl<'de> Visitor<'de> for Entries {
            type Value = Rules;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a mapping of rule IDs")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Rules, A::Error> {
                let mut rules = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    rules.push(entry);
                }
                Ok(Rules(rules))
            }
        }
        deserializer.deserialize_map(Entries)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, expecting = "a mapping of rule settings")]
struct Settings {
    disable: Option<bool>,
    ignore: Option<Vec<Ignore>>,
    remap: Option<Remap>,
    config: Option<yaml_serde::Mapping>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Remap {
    severity: Option<Remapped>,
}

#[derive(Deserialize)]
#[serde(try_from = "Value")]
struct Remapped(Severity);

impl TryFrom<Value> for Remapped {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, String> {
        value
            .as_str()
            .and_then(severity)
            .map(Remapped)
            .ok_or_else(|| "`severity` must be informational, low, medium, or high".to_owned())
    }
}

#[derive(Deserialize)]
#[serde(try_from = "Value")]
struct Ignore((String, Option<usize>));

impl TryFrom<Value> for Ignore {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, String> {
        let entry = value.as_str().ok_or("entries must be strings")?;
        ignore_entry(entry).map(Ignore).ok_or_else(|| {
            format!("entry `{entry}` is not `file`, `file:line`, or `file:line:column`")
        })
    }
}

impl From<Settings> for RuleConfig {
    fn from(settings: Settings) -> Self {
        RuleConfig {
            disable: settings.disable.unwrap_or(false),
            ignore: settings
                .ignore
                .into_iter()
                .flatten()
                .map(|Ignore(entry)| entry)
                .collect(),
            severity: settings
                .remap
                .and_then(|remap| remap.severity)
                .map(|Remapped(severity)| severity),
            options: settings.config.map(Value::Mapping),
        }
    }
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
        let file: Option<File> =
            yaml_serde::from_str(&source).map_err(|error| describe(error.to_string()))?;
        let rules = file
            .and_then(|file| file.rules)
            .map_or_else(Vec::new, |Rules(rules)| rules);
        Ok(Self {
            rules: rules
                .into_iter()
                .map(|(id, settings)| (id, settings.map(RuleConfig::from).unwrap_or_default()))
                .collect(),
        })
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
