use std::collections::BTreeSet;

use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Rule, common, runner};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// Key sets from GitHub's workflow syntax reference:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
pub const SCHEMA: Rule = Rule {
    id: "schema-validity",
    summary: "Workflows must use only the keys GitHub's workflow syntax defines.",
    tag: "syntax/schema-validity",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[("actionlint", "syntax-check")],
    configurable: false,
};

// ShellCheck SC2086 (unquoted expansion), limited to GitHub's environment file variables:
// https://www.shellcheck.net/wiki/SC2086
pub const SCRIPT_LINT: Rule = Rule {
    id: "embedded-script-lint",
    summary: "Environment file paths must be quoted in shell scripts.",
    tag: "embedded-script-lint",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[("actionlint", "shellcheck")],
    configurable: false,
};

// zizmor `insecure-url-scheme` and GASH "Unsecured Protocol".
pub const INSECURE_PROTOCOL: Rule = Rule {
    id: "insecure-protocol",
    summary: "URLs must use an encrypted scheme.",
    tag: "insecure-protocol",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "insecure-url-scheme")],
    configurable: false,
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

/// `GITHUB_TOKEN` scopes from the workflow syntax reference, plus `models`
/// (https://github.blog/ai-and-ml/generative-ai/automate-your-project-with-github-models-in-actions/)
/// and `repository-projects`, which the current reference dropped without a documented removal
/// (https://github.blog/changelog/2021-04-20-github-actions-control-permissions-for-github_token/).
const PERMISSION_SCOPES: &[&str] = &[
    "actions",
    "artifact-metadata",
    "attestations",
    "checks",
    "code-quality",
    "contents",
    "deployments",
    "discussions",
    "id-token",
    "issues",
    "models",
    "packages",
    "pages",
    "pull-requests",
    "repository-projects",
    "security-events",
    "statuses",
    "vulnerability-alerts",
];

/// https://docs.github.com/en/actions/reference/workflows-and-actions/reusable-workflows
const CALLER_KEYS: &[&str] = &[
    "name",
    "uses",
    "with",
    "secrets",
    "strategy",
    "needs",
    "if",
    "concurrency",
    "permissions",
    "cache-mode",
];

/// "The maximum number of top-level properties for `inputs` is 25."
const MAX_DISPATCH_INPUTS: usize = 25;

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
    let mut scopes = vec![(Route::default(), "workflow".to_owned(), &workflow.value)];
    for job in common::jobs(workflow) {
        scopes.push((job.route.clone(), job.location(), job.value));
        for (route, key) in unknown_keys(job.value, JOB_KEYS, &job.route) {
            problems.push((route, job.location(), format!("unknown job key `{key}`")));
        }
        if job.get("uses").is_some() {
            for (route, key) in unknown_keys(job.value, CALLER_KEYS, &job.route)
                .into_iter()
                .filter(|(_, key)| JOB_KEYS.contains(key))
            {
                problems.push((
                    route,
                    job.location(),
                    format!("a job that calls a reusable workflow cannot set `{key}`"),
                ));
            }
        }
        // The context availability table lists no `secrets` for `jobs.<id>.if` or `steps.if`:
        // https://docs.github.com/en/actions/reference/workflows-and-actions/contexts
        let conditions = std::iter::once((job.route.clone(), job.location(), job.value)).chain(
            common::steps(&job)
                .into_iter()
                .map(|step| (step.route.clone(), step.location.clone(), step.value)),
        );
        for (route, location, value) in conditions {
            if value
                .get("if")
                .and_then(Value::as_str)
                .is_some_and(references_secrets)
            {
                problems.push((
                    route.with_key("if"),
                    location,
                    "`if` cannot read `secrets`; set an `env` value from the secret and test that"
                        .to_owned(),
                ));
            }
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
    for trigger in common::triggers(workflow) {
        let inputs = trigger
            .config
            .and_then(|config| config.get("inputs"))
            .and_then(Value::as_mapping);
        if trigger.name == "workflow_dispatch"
            && let Some(inputs) = inputs
            && inputs.len() > MAX_DISPATCH_INPUTS
        {
            problems.push((
                trigger.route.with_key("inputs"),
                "workflow".to_owned(),
                format!(
                    "`workflow_dispatch` allows at most {MAX_DISPATCH_INPUTS} inputs, found {}",
                    inputs.len()
                ),
            ));
        }
    }
    for (route, location, scope) in &scopes {
        // "The combination of `queue: max` and `cancel-in-progress: true` is not allowed."
        let Some(concurrency) = scope.get("concurrency") else {
            continue;
        };
        if concurrency.get("queue").and_then(Value::as_str) == Some("max")
            && concurrency
                .get("cancel-in-progress")
                .and_then(Value::as_bool)
                == Some(true)
        {
            problems.push((
                route.with_key("concurrency"),
                location.clone(),
                "`queue: max` cannot be combined with `cancel-in-progress: true`".to_owned(),
            ));
        }
    }
    for (route, location, scope) in scopes {
        let Some(permissions) = scope.get("permissions") else {
            continue;
        };
        let route = route.with_key("permissions");
        for (route, key) in unknown_keys(permissions, PERMISSION_SCOPES, &route) {
            problems.push((
                route,
                location.clone(),
                format!("unknown permission `{key}`"),
            ));
        }
    }
    problems
        .into_iter()
        .map(|(route, location, message)| {
            common::finding(workflow, &SCHEMA, route, location, message)
        })
        .collect()
}

/// Whether an `if` expression reads the `secrets` context outside a string literal.
fn references_secrets(condition: &str) -> bool {
    let code: String = condition
        .split('\'')
        .step_by(2)
        .collect::<Vec<_>>()
        .join(" ");
    code.match_indices("secrets").any(|(at, _)| {
        let before = code[..at].bytes().next_back();
        let after = code[at + "secrets".len()..].bytes().next();
        !before.is_some_and(|byte| byte.is_ascii_alphanumeric() || b"_-.".contains(&byte))
            && matches!(after, Some(b'.' | b'['))
    })
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
        // `run` defaults to pwsh on Windows and to bash elsewhere, so a matrix over both OSes
        // still runs bash on some entries:
        // https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idstepsshell
        let labels: Vec<String> = runner::job_labels(&job)
            .iter()
            .map(|label| label.to_ascii_lowercase())
            .collect();
        let windows = labels.iter().any(|label| label.starts_with("windows"))
            && !labels.iter().any(|label| {
                ["ubuntu", "macos", "linux"]
                    .iter()
                    .any(|os| label.starts_with(os))
            });
        let job_shell = default_shell(job.value)
            .or(workflow_shell)
            .or(windows.then_some("pwsh"));
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

/// Loopback and bind-all addresses (`+` and `*` are ASP.NET and HTTP.sys listen wildcards).
const LOCAL_HOSTS: &[&str] = &["localhost", "127.0.0.1", "0.0.0.0", "::1", "+", "*"];

/// Whether plain HTTP to `host` stays on the runner: a local address, or a single DNS label such
/// as a service container, which public DNS never resolves.
fn is_local(host: &str) -> bool {
    LOCAL_HOSTS.contains(&host)
        || (!host.is_empty()
            && host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'))
}

pub fn insecure_protocol(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut seen = BTreeSet::new();
    let mut flagged = Vec::new();
    common::strings(&workflow.value, &Route::default(), &mut |route, text| {
        let insecure = ["http://", "git://", "ftp://"].iter().any(|scheme| {
            text.match_indices(scheme).any(|(index, _)| {
                let rest = &text[index + scheme.len()..];
                let host = match rest.strip_prefix('[') {
                    Some(ipv6) => ipv6.split(']').next(),
                    None => rest.split(['/', ':', '"', '\'', ' ', '\n']).next(),
                };
                *scheme != "http://" || !is_local(host.unwrap_or(""))
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
