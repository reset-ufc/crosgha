//! `crosgha.yml`, or an existing `zizmor.yml`, with zizmor's schema and discovery rules so a
//! project can switch tools without rewriting its configuration:
//! https://docs.zizmor.sh/configuration/

use std::{
    fs,
    path::{Path, PathBuf},
};

use yaml_serde::Value;

use crate::{policy::Severity, rules::Rule};

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

impl Config {
    /// The first config file found walking up from `input`, stopping at the Git root. A
    /// `.github/workflows` input starts two levels up so a workflow named `zizmor.yml` is never
    /// read as configuration.
    pub fn discover(input: &Path) -> Result<Option<(PathBuf, Self)>, String> {
        let input = fs::canonicalize(input)
            .map_err(|error| format!("cannot resolve `{}`: {error}", input.display()))?;
        let workflows_dir = input.ends_with(".github/workflows");
        let mut current = match (input.is_dir(), workflows_dir) {
            (true, true) => input.parent().and_then(Path::parent),
            (true, false) => Some(input.as_path()),
            (false, _) => input.parent(),
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
                    let mut parts = entry.split(':');
                    let file = parts.next().unwrap_or_default().to_owned();
                    (file, parts.next().and_then(|line| line.parse().ok()))
                })
                .collect();
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
                },
            ));
            if settings.get("config").is_some() {
                eprintln!(
                    "warning: {}",
                    describe(format!("`rules.{id}.config` is not supported yet"))
                );
            }
        }
        Ok(config)
    }

    fn entries<'a>(&'a self, rule: &'a Rule) -> impl Iterator<Item = &'a RuleConfig> + 'a {
        self.rules
            .iter()
            .filter(|(id, _)| rule.answers_to(id))
            .map(|(_, settings)| settings)
    }

    pub fn disabled(&self, rule: &Rule) -> bool {
        self.entries(rule).any(|settings| settings.disable)
    }

    pub fn ignores(&self, rule: &Rule, file: &str, line: usize) -> bool {
        self.entries(rule).any(|settings| {
            settings
                .ignore
                .iter()
                .any(|(name, at)| name == file && at.is_none_or(|at| at == line))
        })
    }

    pub fn severity(&self, rule: &Rule) -> Option<Severity> {
        self.entries(rule).find_map(|settings| settings.severity)
    }
}
