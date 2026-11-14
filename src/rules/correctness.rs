use std::collections::BTreeSet;
use std::ops::Range;

use yaml_serde::Value;
use yamlpath::Route;

use super::{ChangeKind, Edit, Finding, LineRewrite, ProposedFix, Rule, common, expression, shell};
use crate::policy::{Persona, Severity};
use crate::workflow::Workflow;

// Key sets from GitHub's workflow syntax reference:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
pub const SCHEMA: Rule = Rule {
    id: "schema-validity",
    summary: "Workflows and actions must use only the keys GitHub's syntax defines.",
    tag: "syntax/schema-validity",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[
        ("actionlint", "syntax-check"),
        ("actionlint", "expression"),
        ("ghlint", "JsonSchemaValidation"),
    ],
    configurable: false,
    relational: false,
};

// ShellCheck SC2086 (unquoted expansion) and SC2250 (braces), limited to GitHub's environment file
// variables, and `>` writes that truncate them, after ghlint's `SafeEnvironmentFileRedirect` and
// `EnvironmentFileOverwritten` at commit 74115a4:
// https://www.shellcheck.net/wiki/SC2086
// https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/SafeEnvironmentFileRedirectRule.kt
// https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/EnvironmentFileOverwriteRule.kt
pub const SCRIPT_LINT: Rule = Rule {
    id: "embedded-script-lint",
    summary: "Shell scripts must quote, brace, and append to environment file paths.",
    tag: "embedded-script-lint",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[
        ("actionlint", "shellcheck"),
        ("ghlint", "SafeEnvironmentFileRedirect"),
        ("ghlint", "EnvironmentFileOverwritten"),
    ],
    configurable: false,
    relational: false,
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
    relational: false,
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

/// Keys of `run-step` and `uses-step`, the two kinds of composite step in GitHub's action schema
/// (actions/languageservices `workflow-parser/src/action-v1.0.json` at
/// 4043eda158e16579cc5fb1b0b07a4bce2a76f0b5), where `shell` is required next to `run`.
const RUN_STEP_KEYS: &[&str] = &[
    "name",
    "id",
    "if",
    "run",
    "shell",
    "env",
    "continue-on-error",
    "working-directory",
];
const USES_STEP_KEYS: &[&str] = &[
    "name",
    "id",
    "if",
    "uses",
    "with",
    "env",
    "continue-on-error",
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

/// `step-uses` and a job's `uses` declare no expression context in GitHub's workflow schema
/// (`workflow-v1.0.json`, same commit as above), so `${{ }}` there is refused.
const USES_EXPRESSION: &str = "`uses` cannot contain a `${{ }}` expression";

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
        if job
            .get("uses")
            .and_then(Value::as_str)
            .is_some_and(|uses| uses.contains("${{"))
        {
            problems.push((
                job.route.with_key("uses"),
                job.location(),
                USES_EXPRESSION.to_owned(),
            ));
        }
        // `convertSteps` builds one `IdBuilder` per list, so a `parallel` block is its own scope.
        // IDs cannot hold a dot, so what precedes one in a nested step's location names its block.
        let mut ids = BTreeSet::new();
        for step in common::steps(&job) {
            for (route, key) in unknown_keys(step.value, STEP_KEYS, &step.route) {
                problems.push((
                    route,
                    step.location.clone(),
                    format!("unknown step key `{key}`"),
                ));
            }
            let block = step.location.split_once('.').map(|(block, _)| block);
            if let Some(id) = step.value.get("id").and_then(Value::as_str)
                && !ids.insert((block.map(str::to_owned), id))
            {
                problems.push((
                    step.key("id"),
                    step.location.clone(),
                    format!("step id `{id}` is already used in this list of steps"),
                ));
            }
            if step.uses().is_some_and(|uses| uses.contains("${{")) {
                problems.push((
                    step.key("uses"),
                    step.location.clone(),
                    USES_EXPRESSION.to_owned(),
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
    problems.extend(refused_needs(&common::jobs(workflow)));
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

/// An action file's required keys, its runtime, and its composite steps. `name` and `description`
/// are required by the metadata syntax
/// (https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax), and the
/// runner refuses an unknown `using`, a JavaScript action without `main`, a Docker action without
/// `image`, and a composite action without `steps` (`ActionManifestManager.cs` in actions/runner at
/// 8cfa1049f86781d141056082d73802d56cd2b433).
pub fn action_schema(action: &Workflow) -> Result<Vec<Finding>, String> {
    let mut problems: Vec<(Route<'static>, String, String)> = Vec::new();
    let runs = Route::default().with_key("runs");
    // A missing key has no node of its own, so the finding sits on `runs`, which parsing requires.
    for key in ["name", "description"] {
        if action.value.get(key).is_none() {
            problems.push((
                runs.clone(),
                "action".to_owned(),
                format!("action has no `{key}`"),
            ));
        }
    }
    let using = action.using().unwrap_or_default();
    let required = match using.as_str() {
        "composite" => Some("steps"),
        "docker" => Some("image"),
        "node12" | "node16" | "node20" | "node24" => Some("main"),
        _ => {
            problems.push((
                runs.with_key("using"),
                "action".to_owned(),
                format!(
                    "`using: {using}` is not one of composite, docker, node12, node16, node20, or node24"
                ),
            ));
            None
        }
    };
    let declared = |key: &str| action.value.get("runs").and_then(|runs| runs.get(key));
    if let Some(key) = required {
        let present = match declared(key) {
            Some(Value::String(text)) => key != "steps" && !text.is_empty(),
            Some(Value::Sequence(_)) => key == "steps",
            _ => false,
        };
        if !present {
            let form = if key == "steps" {
                "a list"
            } else {
                "a non-empty string"
            };
            problems.push((
                runs.clone(),
                "action".to_owned(),
                format!("`using: {using}` needs `runs.{key}` as {form}"),
            ));
        }
    }
    for job in common::jobs(action) {
        for step in common::steps(&job) {
            let (run, uses) = (step.value.get("run"), step.value.get("uses"));
            let known = match (run, uses) {
                (Some(_), None) => RUN_STEP_KEYS,
                (None, Some(_)) => USES_STEP_KEYS,
                _ => {
                    problems.push((
                        step.route.clone(),
                        step.location.clone(),
                        "composite step needs exactly one of `uses` or `run`".to_owned(),
                    ));
                    continue;
                }
            };
            let kind = if run.is_some() { "run" } else { "uses" };
            for (route, key) in unknown_keys(step.value, known, &step.route) {
                problems.push((
                    route,
                    step.location.clone(),
                    format!("a composite `{kind}` step cannot set `{key}`"),
                ));
            }
            if run.is_some() && step.value.get("shell").is_none() {
                problems.push((
                    step.key("run"),
                    step.location.clone(),
                    "composite `run` steps need `shell`".to_owned(),
                ));
            }
        }
    }
    problems
        .into_iter()
        .map(|(route, location, message)| {
            common::finding(action, &SCHEMA, route, location, message)
        })
        .collect()
}

/// The names in a job's `needs`, a string or a list of strings, with the route of each.
fn needs<'a>(job: &common::Job<'a>) -> Vec<(Route<'static>, &'a str)> {
    let route = job.route.with_key("needs");
    match job.get("needs") {
        Some(Value::String(name)) => vec![(route, name.as_str())],
        Some(Value::Sequence(names)) => names
            .iter()
            .enumerate()
            .filter_map(|(index, name)| Some((route.with_key(index), name.as_str()?)))
            .collect(),
        _ => Vec::new(),
    }
}

/// `needs` GitHub refuses: an unknown job, or a job that never starts because of a cycle. As in
/// `validateNeeds` (actions/languageservices `workflow-parser/src/model/converter/jobs.ts` at
/// 4043eda158e16579cc5fb1b0b07a4bce2a76f0b5), names compare exactly and jobs start once their needs
/// have. An unknown name does not hold a job back here, so only the wrong `needs` is reported.
fn refused_needs(jobs: &[common::Job<'_>]) -> Vec<(Route<'static>, String, String)> {
    let names: BTreeSet<&str> = jobs.iter().map(|job| job.name).collect();
    let mut started = BTreeSet::new();
    loop {
        let before = started.len();
        for job in jobs {
            if needs(job)
                .iter()
                .all(|(_, need)| !names.contains(need) || started.contains(need))
            {
                started.insert(job.name);
            }
        }
        if started.len() == before {
            break;
        }
    }
    let mut problems = Vec::new();
    for job in jobs {
        for (route, need) in needs(job) {
            let message = if !names.contains(need) {
                format!("`needs` names `{need}`, which is not a job in this workflow")
            } else if !started.contains(need) {
                format!("`needs` names `{need}`, which never starts because of a dependency cycle")
            } else {
                continue;
            };
            problems.push((route, job.location(), message));
        }
    }
    problems
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
        for expression in expression::expressions(text) {
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

/// Property names read as `<context>.<name>` or `<context>['<name>']` in an expression, outside
/// other string literals.
pub(super) fn context_names(expression: &str, context: &str) -> Vec<String> {
    let prefix = format!("{context}.");
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    // Even parts are code; odd parts are the contents of string literals.
    expression::dotted(expression)
        .split('\'')
        .step_by(2)
        .flat_map(|code| {
            code.match_indices(prefix.as_str())
                .filter(|(at, _)| !code[..*at].ends_with(|c: char| is_name(c) || c == '.'))
                .map(|(at, _)| {
                    let rest = &code[at + prefix.len()..];
                    rest[..rest.find(|c: char| !is_name(c)).unwrap_or(rest.len())].to_owned()
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

/// Environment files that `run` scripts write to:
/// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#environment-files
const ENV_FILES: &[&str] = &[
    "GITHUB_ENV",
    "GITHUB_OUTPUT",
    "GITHUB_PATH",
    "GITHUB_STATE",
    "GITHUB_STEP_SUMMARY",
];

/// GitHub documents `>` as the way to clear a step's summary, so an overwrite there may be meant:
/// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#overwriting-job-summaries
const CLEARABLE: &str = "GITHUB_STEP_SUMMARY";

pub fn script_lint(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some(script) = step.run() else { continue };
            if !shell::runs_posix(workflow, &job, &step) {
                continue;
            }
            let lines: Vec<&str> = script.lines().collect();
            let message = if lines.iter().any(|line| unquoted_env_file(line)) {
                Some("quote environment file paths to prevent word splitting (SC2086)")
            } else if lines.iter().any(|line| unbraced_write(line)) {
                Some("brace environment file paths, as in `\"${GITHUB_OUTPUT}\"` (SC2250)")
            } else {
                None
            };
            if let Some(message) = message {
                let mut finding = common::finding(
                    workflow,
                    &SCRIPT_LINT,
                    step.key("run"),
                    step.location.clone(),
                    message,
                )?;
                finding.concepts = &["SafeEnvironmentFileRedirect"];
                finding.fix = quote_env_files(workflow, &step, script);
                findings.push(finding);
            }
            let mut overwritten: Vec<&str> = Vec::new();
            for line in &lines {
                for write in writes(line) {
                    if write.overwrite && !overwritten.contains(&write.name) {
                        overwritten.push(write.name);
                    }
                }
            }
            if !overwritten.is_empty() {
                let mut finding = common::finding(
                    workflow,
                    &SCRIPT_LINT,
                    step.key("run"),
                    step.location.clone(),
                    format!(
                        "`>` overwrites environment file `{}`; append with `>>`",
                        overwritten.join("`, `")
                    ),
                )?;
                finding.concepts = &["EnvironmentFileOverwritten"];
                finding.fix = append_env_files(workflow, &step, script);
                findings.push(finding);
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

/// A redirect to an environment file on one line, read loosely as ghlint's regexes read it.
struct EnvFileWrite {
    name: &'static str,
    /// `>` rather than `>>`.
    overwrite: bool,
    quoted: bool,
    braced: bool,
}

/// The writes on `line`: `>` or `>>` outside double quotes, blanks, then `$NAME`, `${NAME}`,
/// `"$NAME"`, or `"${NAME}"` as a whole word. Like ghlint, this counts `2>`, `&>`, and `<>`, and
/// not `>|`; unlike ghlint, it also reads `GITHUB_STATE` and skips targets such as `$NAME.bak`.
fn writes(line: &str) -> Vec<EnvFileWrite> {
    let mut found = Vec::new();
    for (index, _) in line.match_indices('>') {
        let before = &line[..index];
        if before.ends_with('>') || !before.matches('"').count().is_multiple_of(2) {
            continue;
        }
        let after = &line[index + 1..];
        let append = after.starts_with('>');
        let target = after[usize::from(append)..].trim_start_matches([' ', '\t']);
        let quoted = target.starts_with('"');
        let Some(expansion) = target[usize::from(quoted)..].strip_prefix('$') else {
            continue;
        };
        let braced = expansion.starts_with('{');
        let expansion = &expansion[usize::from(braced)..];
        let found_name = ENV_FILES.iter().find_map(|name| {
            let rest = expansion.strip_prefix(name)?;
            let rest = match braced {
                true => rest.strip_prefix('}')?,
                false => rest,
            };
            let rest = match quoted {
                true => rest.strip_prefix('"')?,
                false => rest,
            };
            let word_end = rest
                .chars()
                .next()
                .is_none_or(|next| " \t;&|)<>".contains(next));
            word_end.then_some(*name)
        });
        if let Some(name) = found_name {
            found.push(EnvFileWrite {
                name,
                overwrite: !append,
                quoted,
                braced,
            });
        }
    }
    found
}

/// Whether `line` redirects to a quoted environment file path without braces, which ghlint's
/// `SafeEnvironmentFileRedirect` reports after ShellCheck SC2250:
/// https://www.shellcheck.net/wiki/SC2250
fn unbraced_write(line: &str) -> bool {
    writes(line)
        .iter()
        .any(|write| write.quoted && !write.braced)
}

/// The environment file a redirect target such as `"${GITHUB_OUTPUT}"` names.
fn target_name(word: &str) -> &str {
    word.trim_matches(['"', '$', '{', '}'])
}

/// Quotes and braces each environment file path that `script` redirects to, so
/// `>> $GITHUB_OUTPUT` and `>> "$GITHUB_OUTPUT"` become `>> "${GITHUB_OUTPUT}"`, the form ghlint's
/// `SafeEnvironmentFileRedirect` asks for. `None` when the rule would still report a path that is
/// not a redirect target (a command argument, a `for` list, a pattern, text in quotes), or when
/// the YAML scalar is not the script as written.
fn quote_env_files(workflow: &Workflow, step: &common::Step, script: &str) -> Option<ProposedFix> {
    if !shell::verbatim(workflow, step, script) {
        return None;
    }
    let mut replacements = Vec::new();
    for redirect in shell::redirects(script, ENV_FILES)? {
        let word = &script[redirect.target.clone()];
        let canonical = format!("\"${{{}}}\"", target_name(word));
        if word != canonical {
            replacements.push((redirect.target, canonical));
        }
    }
    let quoted = replace(script, &replacements);
    let reported = quoted
        .lines()
        .any(|line| unquoted_env_file(line) || unbraced_write(line));
    if replacements.is_empty() || reported {
        return None;
    }
    line_rewrites(step, script, &quoted)
}

/// Turns each `>` that truncates an environment file into `>>`. The runner gives every step empty
/// environment files, so the scripts differ only once the step has written to the file before:
/// a loop, an earlier line, or a program it ran. Those writes were lost and now reach the runner,
/// which is what `>>` is for. `None` when the rule would still report an overwrite: one of the
/// step summary, which `>` may clear on purpose; `&>`, which sh reads as `&` and `>`; `<>`; or a
/// `>` the reader does not take for a redirect. A script with `[[` or `$[` is left alone, because
/// a `>` there compares and the reader does not track them.
// Per-step files: https://github.com/actions/runner/blob/8cfa1049f86781d141056082d73802d56cd2b433/src/Runner.Worker/FileCommandManager.cs
fn append_env_files(workflow: &Workflow, step: &common::Step, script: &str) -> Option<ProposedFix> {
    if !shell::verbatim(workflow, step, script) || script.contains("[[") || script.contains("$[") {
        return None;
    }
    let mut replacements = Vec::new();
    for redirect in shell::redirects(script, ENV_FILES)? {
        let before = script[..redirect.operator.start].bytes().next_back();
        if &script[redirect.operator.clone()] == ">"
            && !matches!(before, Some(b'<' | b'&'))
            && target_name(&script[redirect.target]) != CLEARABLE
        {
            replacements.push((redirect.operator, ">>".to_owned()));
        }
    }
    let appended = replace(script, &replacements);
    let reported = appended
        .lines()
        .any(|line| writes(line).iter().any(|write| write.overwrite));
    if replacements.is_empty() || reported {
        return None;
    }
    line_rewrites(step, script, &appended)
}

/// `script` with each range replaced by its text; the ranges are in order and do not overlap.
fn replace(script: &str, replacements: &[(Range<usize>, String)]) -> String {
    let mut replaced = String::with_capacity(script.len() + 4 * replacements.len());
    let mut copied = 0;
    for (range, text) in replacements {
        replaced.push_str(&script[copied..range.start]);
        replaced.push_str(text);
        copied = range.end;
    }
    replaced.push_str(&script[copied..]);
    replaced
}

/// The rewrite of the step's `run` scalar from `script` to `rewritten`, line by line. `None` when
/// an unchanged line has the text of a changed one, because `Edit::Lines` rewrites the next line
/// with that text.
fn line_rewrites(step: &common::Step, script: &str, rewritten: &str) -> Option<ProposedFix> {
    let mut rewrites = Vec::new();
    let mut unchanged = Vec::new();
    for (before, after) in script.split('\n').zip(rewritten.split('\n')) {
        match before == after {
            true => unchanged.push(before.trim()),
            false => rewrites.push(LineRewrite {
                from: before.trim().to_owned(),
                to: after.trim().to_owned(),
            }),
        }
    }
    if rewrites
        .iter()
        .any(|rewrite| unchanged.contains(&rewrite.from.as_str()))
    {
        return None;
    }
    Some(ProposedFix {
        edit: Edit::Lines(rewrites),
        changed_route: step.key("run"),
        change: ChangeKind::Replace,
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
                workflow.kind.noun(),
                "URL uses an unencrypted scheme",
            )
        })
        .collect()
}
