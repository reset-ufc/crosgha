use std::collections::BTreeSet;

use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// Key sets from GitHub's workflow syntax reference:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
pub const SCHEMA: Rule = Rule {
    id: "schema-validity",
    tag: "syntax/schema-validity",
    severity: Severity::Error,
    persona: Persona::Regular,
    aliases: &["syntax-check"],
};

// ShellCheck SC2086 (unquoted expansion), limited to GitHub's environment file variables:
// https://www.shellcheck.net/wiki/SC2086
pub const SCRIPT_LINT: Rule = Rule {
    id: "embedded-script-lint",
    tag: "embedded-script-lint",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    aliases: &["shellcheck"],
};

// zizmor `insecure-url-scheme` and GASH "Unsecured Protocol".
pub const INSECURE_PROTOCOL: Rule = Rule {
    id: "insecure-protocol",
    tag: "insecure-protocol",
    severity: Severity::Warning,
    persona: Persona::Regular,
    aliases: &["insecure-url-scheme"],
};

const WORKFLOW_KEYS: &[&str] = &[
    "name",
    "run-name",
    "on",
    "permissions",
    "env",
    "defaults",
    "concurrency",
    "cache-mode",
    "jobs",
];

const JOB_KEYS: &[&str] = &[
    "name",
    "permissions",
    "needs",
    "if",
    "runs-on",
    "snapshot",
    "environment",
    "concurrency",
    "cache-mode",
    "outputs",
    "env",
    "defaults",
    "steps",
    "timeout-minutes",
    "strategy",
    "continue-on-error",
    "container",
    "services",
    "uses",
    "with",
    "secrets",
];

const STEP_KEYS: &[&str] = &[
    "id",
    "if",
    "name",
    "uses",
    "run",
    "working-directory",
    "shell",
    "with",
    "env",
    "continue-on-error",
    "timeout-minutes",
    "background",
    "wait",
    "wait-all",
    "cancel",
    "parallel",
];

/// A step does exactly one of these.
const STEP_ACTIONS: &[&str] = &["uses", "run", "parallel", "wait", "wait-all", "cancel"];

fn unknown_keys<'a>(
    value: &'a Value,
    known: &[&str],
    route: &Route<'static>,
) -> Vec<(Route<'static>, &'a str)> {
    value
        .as_mapping()
        .into_iter()
        .flatten()
        .filter_map(|(key, _)| key.as_str())
        .filter(|key| !known.contains(key))
        .map(|key| (route.with_key(key.to_owned()), key))
        .collect()
}

pub fn schema(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut problems: Vec<(Route<'static>, String, String)> = Vec::new();
    for (route, key) in unknown_keys(&workflow.value, WORKFLOW_KEYS, &Route::default()) {
        problems.push((
            route,
            "workflow".to_owned(),
            format!("unknown workflow key `{key}`"),
        ));
    }
    for job in common::jobs(workflow) {
        for (route, key) in unknown_keys(job.value, JOB_KEYS, &job.route) {
            problems.push((route, job.location(), format!("unknown job key `{key}`")));
        }
        if job.get("runs-on").is_none() && job.get("uses").is_none() {
            problems.push((
                job.route.clone(),
                job.location(),
                "job needs `runs-on` or `uses`".to_owned(),
            ));
        }
        for step in common::steps(&job) {
            for (route, key) in unknown_keys(step.value, STEP_KEYS, &step.route) {
                problems.push((
                    route,
                    step.location.clone(),
                    format!("unknown step key `{key}`"),
                ));
            }
            let actions = STEP_ACTIONS
                .iter()
                .filter(|key| step.value.get(**key).is_some())
                .count();
            if actions != 1 {
                problems.push((
                    step.route.clone(),
                    step.location.clone(),
                    "step needs exactly one of `uses`, `run`, `parallel`, `wait`, `wait-all`, or `cancel`"
                        .to_owned(),
                ));
            }
        }
    }
    problems
        .into_iter()
        .map(|(route, location, message)| {
            common::finding(workflow, &SCHEMA, route, location, message)
        })
        .collect()
}

const ENV_FILES: &[&str] = &[
    "GITHUB_ENV",
    "GITHUB_OUTPUT",
    "GITHUB_PATH",
    "GITHUB_STATE",
    "GITHUB_STEP_SUMMARY",
];

fn default_shell(scope: &Value) -> Option<&str> {
    scope
        .get("defaults")
        .and_then(|defaults| defaults.get("run"))
        .and_then(|run| run.get("shell"))
        .and_then(Value::as_str)
}

pub fn script_lint(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let workflow_shell = default_shell(&workflow.value);
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        let job_shell = default_shell(job.value).or(workflow_shell);
        for step in common::steps(&job) {
            let Some(script) = step.run() else { continue };
            let shell = step
                .value
                .get("shell")
                .and_then(Value::as_str)
                .or(job_shell)
                .unwrap_or("bash");
            let posix = ["bash", "sh"]
                .iter()
                .any(|name| shell.split_whitespace().next() == Some(name));
            if posix && script.lines().any(unquoted_env_file) {
                findings.push(common::finding(
                    workflow,
                    &SCRIPT_LINT,
                    step.key("run"),
                    step.location.clone(),
                    "quote environment file paths to prevent word splitting (SC2086)",
                )?);
            }
        }
    }
    Ok(findings)
}

fn unquoted_env_file(line: &str) -> bool {
    ENV_FILES.iter().any(|name| {
        [format!("${name}"), format!("${{{name}}}")]
            .iter()
            .any(|expansion| {
                line.match_indices(expansion.as_str()).any(|(index, _)| {
                    let quotes = line[..index].matches('"').count();
                    let whole_name =
                        !line[index + expansion.len()..].starts_with(|character: char| {
                            character.is_ascii_alphanumeric() || character == '_'
                        });
                    quotes.is_multiple_of(2) && whole_name
                })
            })
    })
}

const LOCAL_HOSTS: &[&str] = &["localhost", "127.0.0.1", "0.0.0.0", "[::1]"];

pub fn insecure_protocol(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut seen = BTreeSet::new();
    let mut flagged = Vec::new();
    common::strings(&workflow.value, &Route::default(), &mut |route, text| {
        let insecure = ["http://", "git://", "ftp://"].iter().any(|scheme| {
            text.match_indices(scheme).any(|(index, _)| {
                let host = text[index + scheme.len()..]
                    .split(['/', ':', '"', '\'', ' ', '\n'])
                    .next()
                    .unwrap_or("");
                *scheme != "http://" || !LOCAL_HOSTS.contains(&host)
            })
        });
        if insecure && seen.insert(format!("{route:?}")) {
            flagged.push(route.clone());
        }
    });
    flagged
        .into_iter()
        .map(|route| {
            common::finding(
                workflow,
                &INSECURE_PROTOCOL,
                route,
                "workflow",
                "URL uses an unencrypted scheme",
            )
        })
        .collect()
}
