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
    equivalents: &[("actionlint", "syntax-check"), ("actionlint", "expression")],
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

/// `GITHUB_TOKEN` scopes and the levels each accepts, from `permissions-mapping` in GitHub's
/// published workflow schema (actions/languageservices `workflow-parser/src/workflow-v1.0.json`
/// at 4043eda158e16579cc5fb1b0b07a4bce2a76f0b5). `read-all`, `write-all`, and `{}` are the only
/// shorthands.
const PERMISSION_SCOPES: &[&str] = &[
    "actions",
    "artifact-metadata",
    "attestations",
    "checks",
    "code-quality",
    "contents",
    "copilot-requests",
    "deployments",
    "discussions",
    "drives",
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

fn permission_levels(scope: &str) -> &'static [&'static str] {
    match scope {
        "copilot-requests" | "id-token" => &["write", "none"],
        "models" | "vulnerability-alerts" => &["read", "none"],
        _ => &["read", "write", "none"],
    }
}

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
        problems.extend(undefined_references(&job));
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
        if let Some(shorthand) = permissions.as_str()
            && !matches!(shorthand, "read-all" | "write-all")
        {
            problems.push((
                route.clone(),
                location.clone(),
                format!(
                    "`permissions: {shorthand}` must be `read-all`, `write-all`, or a map of scopes"
                ),
            ));
        }
        for (route, key) in unknown_keys(permissions, PERMISSION_SCOPES, &route) {
            problems.push((
                route,
                location.clone(),
                format!("unknown permission `{key}`"),
            ));
        }
        for (scope, level) in permissions.as_mapping().into_iter().flatten() {
            let Some(scope) = scope
                .as_str()
                .filter(|scope| PERMISSION_SCOPES.contains(scope))
            else {
                continue;
            };
            let levels = permission_levels(scope);
            if !level.as_str().is_some_and(|level| levels.contains(&level)) {
                problems.push((
                    route.with_key(scope.to_owned()),
                    location.clone(),
                    format!("`{scope}` accepts only {}", levels.join(", ")),
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

/// `matrix.<key>` and `steps.<id>` references in a job that name no key of its matrix and no step
/// `id`. A matrix built from an expression is skipped, since its keys are known only at run time.
fn undefined_references(job: &common::Job<'_>) -> Vec<(Route<'static>, String, String)> {
    let matrix = job
        .get("strategy")
        .and_then(|strategy| strategy.get("matrix"));
    let matrix_keys: Option<Vec<String>> = match matrix {
        None => Some(Vec::new()),
        Some(Value::Mapping(matrix)) => {
            let include = match matrix.get("include") {
                None => Some(Vec::new()),
                Some(Value::Sequence(entries)) => entries
                    .iter()
                    .map(|entry| {
                        entry
                            .as_mapping()
                            .map(|entry| entry.keys().collect::<Vec<_>>())
                    })
                    .collect::<Option<Vec<_>>>()
                    .map(|keys| keys.concat()),
                Some(_) => None,
            };
            include.map(|include| {
                matrix
                    .keys()
                    .chain(include)
                    .filter_map(Value::as_str)
                    .filter(|key| !matches!(*key, "include" | "exclude"))
                    .map(str::to_ascii_lowercase)
                    .collect()
            })
        }
        Some(_) => None,
    };
    let step_ids: Vec<String> = common::steps(job)
        .iter()
        .filter_map(|step| step.value.get("id").and_then(Value::as_str))
        .map(str::to_ascii_lowercase)
        .collect();

    let mut expressions: Vec<(Route<'static>, &str)> = Vec::new();
    common::strings(job.value, &job.route, &mut |route, text| {
        for expression in common::expressions(text) {
            expressions.push((route.clone(), expression));
        }
    });
    // `if` is an expression even without `${{ }}`.
    let conditions = std::iter::once((job.route.clone(), job.value)).chain(
        common::steps(job)
            .into_iter()
            .map(|step| (step.route.clone(), step.value)),
    );
    for (route, value) in conditions {
        if let Some(condition) = value.get("if").and_then(Value::as_str)
            && !condition.contains("${{")
        {
            expressions.push((route.with_key("if"), condition));
        }
    }

    let mut problems = Vec::new();
    for (route, expression) in expressions {
        if let Some(keys) = &matrix_keys {
            for key in context_names(expression, "matrix") {
                if !keys.contains(&key.to_ascii_lowercase()) {
                    problems.push((
                        route.clone(),
                        job.location(),
                        format!("`matrix.{key}` is not a key of this job's matrix"),
                    ));
                }
            }
        }
        for id in context_names(expression, "steps") {
            if !step_ids.contains(&id.to_ascii_lowercase()) {
                problems.push((
                    route.clone(),
                    job.location(),
                    format!("`steps.{id}` names no step `id` in this job"),
                ));
            }
        }
    }
    problems
}

/// Property names read as `<context>.<name>` in an expression, outside string literals.
pub(super) fn context_names<'a>(expression: &'a str, context: &str) -> Vec<&'a str> {
    let prefix = format!("{context}.");
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    expression
        .split('\'')
        .step_by(2)
        .flat_map(|code| {
            code.match_indices(prefix.as_str())
                .filter(|(index, _)| !code[..*index].ends_with(|c: char| is_name(c) || c == '.'))
                .map(|(index, _)| {
                    let rest = &code[index + prefix.len()..];
                    &rest[..rest.find(|c: char| !is_name(c)).unwrap_or(rest.len())]
                })
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
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
            // A custom shell such as `/usr/bin/bash -e {0}` names its program by path.
            let program = shell.split_whitespace().next().unwrap_or("");
            let posix = matches!(program.rsplit('/').next(), Some("bash" | "sh"));
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
