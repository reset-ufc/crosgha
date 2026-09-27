use yaml_serde::{Mapping, Value};
use yamlpath::Route;

use super::{Finding, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GASH (Freitas & Rocha, VEM 2024) "Lack of Error Handling: continue-on-error".
pub const ERROR_SUPPRESSION: Rule = Rule {
    id: "error-suppression",
    summary: "`continue-on-error: true` hides failures.",
    tag: "error-suppression",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
};

// Khatami et al. (SCAM 2024), KZ-06/KZ-07, and zizmor `concurrency-limits`.
pub const CONCURRENCY: Rule = Rule {
    id: "missing-concurrency",
    summary: "CI workflows should cancel superseded runs with a concurrency group.",
    tag: "missing-concurrency",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[("zizmor", "concurrency-limits")],
    configurable: false,
};

// Khatami et al. (SCAM 2024), KZ-08, and Semgrep `github-action-push-without-branches`.
pub const BROAD_TRIGGER: Rule = Rule {
    id: "broad-trigger",
    summary: "push and pull_request triggers should filter branches, tags, or paths.",
    tag: "broad-trigger",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
};

// Khatami et al. (SCAM 2024), KZ-09 "Avoid executing scheduled workflows on forks".
pub const FORK_GUARD: Rule = Rule {
    id: "missing-fork-guard",
    summary: "Scheduled jobs should skip forks.",
    tag: "missing-fork-guard",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
};

// Khatami et al. (SCAM 2024), KZ-19/KZ-22: CI on a single OS or language version.
pub const MATRIX: Rule = Rule {
    id: "missing-matrix",
    summary: "CI should test more than one OS or toolchain version.",
    tag: "missing-matrix/test-diversity",
    severity: Severity::Note,
    persona: Persona::Auditor,
    equivalents: &[],
    configurable: false,
};

// Khatami et al. (SCAM 2024), KZ-17 "Use cache parameter instead of cache option".
pub const CACHE_MISUSE: Rule = Rule {
    id: "cache-misuse",
    summary: "Use the setup action's `cache` input instead of a separate actions/cache step.",
    tag: "cache-misuse",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
};

// GASH (VEM 2024) "Code Replica": a step repeated verbatim within one job.
pub const DUPLICATED_STEPS: Rule = Rule {
    id: "duplicated-steps",
    summary: "A job should not repeat a step verbatim.",
    tag: "redundant/duplicated-steps",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
};

// GASH (VEM 2024) "Long Code Blocks" and "unnecessary complexity"; Khatami et al. KZ-18.
pub const COMPLEXITY: Rule = Rule {
    id: "complex-job",
    summary: "Jobs, scripts, and conditions should stay small enough to review.",
    tag: "long-job/complexity",
    severity: Severity::Note,
    persona: Persona::Auditor,
    equivalents: &[],
    configurable: false,
};

// zizmor `anonymous-definition` and Khatami et al. (SCAM 2024), KZ-16 "Name run steps".
pub const READABILITY: Rule = Rule {
    id: "readability",
    summary: "Workflows and run steps should have names.",
    tag: "readability",
    severity: Severity::Note,
    persona: Persona::Auditor,
    equivalents: &[("zizmor", "anonymous-definition")],
    configurable: false,
};

// GitHub disabled `set-env`/`add-path` and deprecated `set-output`/`save-state`:
// https://github.blog/changelog/2020-10-01-github-actions-deprecating-set-env-and-add-path-commands/
// https://github.blog/changelog/2022-10-11-github-actions-deprecating-save-state-and-set-output-commands/
// GitHub removed the `ACTIONS_ALLOW_USE_UNSECURE_NODE_VERSION` opt-out along with Node 20:
// https://github.blog/changelog/2026-09-23-node-20-is-no-longer-available-in-github-actions/
pub const DEPRECATED: Rule = Rule {
    id: "deprecated-command",
    summary: "Deprecated workflow commands and removed opt-out variables must not be used.",
    tag: "deprecated-command/feature",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[
        ("zizmor", "insecure-commands"),
        ("actionlint", "deprecated-commands"),
    ],
    configurable: false,
};

pub fn error_suppression(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let suppresses = |value: Option<&Value>| {
        matches!(value, Some(Value::Bool(true))) || value.and_then(Value::as_str) == Some("true")
    };
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        if suppresses(job.get("continue-on-error")) {
            findings.push(common::finding(
                workflow,
                &ERROR_SUPPRESSION,
                job.route.with_key("continue-on-error"),
                job.location(),
                "job failures never fail the workflow",
            )?);
        }
        for step in common::steps(&job) {
            if suppresses(step.value.get("continue-on-error")) {
                findings.push(common::finding(
                    workflow,
                    &ERROR_SUPPRESSION,
                    step.key("continue-on-error"),
                    step.location.clone(),
                    "step failures never fail the job",
                )?);
            }
        }
    }
    Ok(findings)
}

pub fn concurrency(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let jobs = common::jobs(workflow);
    if !common::has_trigger(workflow, &["push", "pull_request"])
        || workflow.value.get("concurrency").is_some()
        || jobs.iter().all(|job| job.get("concurrency").is_some())
    {
        return Ok(Vec::new());
    }
    Ok(vec![common::finding(
        workflow,
        &CONCURRENCY,
        Route::default().with_key("on"),
        "workflow",
        "superseded runs keep running without a concurrency group",
    )?])
}

const TRIGGER_FILTERS: &[&str] = &[
    "branches",
    "branches-ignore",
    "tags",
    "tags-ignore",
    "paths",
    "paths-ignore",
];

pub fn broad_trigger(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    common::triggers(workflow)
        .into_iter()
        .filter(|trigger| matches!(trigger.name, "push" | "pull_request"))
        .filter(|trigger| {
            trigger
                .config
                .is_none_or(|config| TRIGGER_FILTERS.iter().all(|key| config.get(key).is_none()))
        })
        .map(|trigger| {
            common::finding(
                workflow,
                &BROAD_TRIGGER,
                trigger.route,
                "workflow",
                format!("`{}` has no branch, tag, or path filter", trigger.name),
            )
        })
        .collect()
}

// Scheduled runs carry the repository object in `github.event` since
// https://github.blog/changelog/2022-09-27-github-actions-additional-information-available-in-github-event-payload-for-scheduled-workflow-runs/
const FORK_GUARDS: &[&str] = &[
    "github.repository",
    "github.event.repository.fork",
    "github.event.repository.full_name",
    "github.event.repository.owner",
];

pub fn fork_guard(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    if !common::has_trigger(workflow, &["schedule"]) {
        return Ok(Vec::new());
    }
    common::jobs(workflow)
        .iter()
        .filter(|job| {
            !job.get("if")
                .and_then(Value::as_str)
                .is_some_and(|condition| FORK_GUARDS.iter().any(|guard| condition.contains(guard)))
        })
        .map(|job| {
            common::finding(
                workflow,
                &FORK_GUARD,
                job.route.clone(),
                job.location(),
                "scheduled job also runs in every fork",
            )
        })
        .collect()
}

pub fn matrix(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let jobs = common::jobs(workflow);
    let runs = jobs.iter().any(|job| job.get("runs-on").is_some());
    let has_matrix = jobs.iter().any(|job| {
        job.get("strategy")
            .and_then(|strategy| strategy.get("matrix"))
            .is_some()
    });
    if !common::has_trigger(workflow, &["push", "pull_request"]) || !runs || has_matrix {
        return Ok(Vec::new());
    }
    Ok(vec![common::finding(
        workflow,
        &MATRIX,
        Route::default().with_key("jobs"),
        "workflow",
        "CI runs on a single runner and toolchain configuration",
    )?])
}

const SETUP_WITH_CACHE: &[&str] = &[
    "actions/setup-node",
    "actions/setup-python",
    "actions/setup-java",
    "actions/setup-go",
    "actions/setup-dotnet",
];

pub fn cache_misuse(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        let steps = common::steps(&job);
        let has_setup = steps
            .iter()
            .any(|step| SETUP_WITH_CACHE.iter().any(|setup| step.uses_action(setup)));
        if !has_setup {
            continue;
        }
        for step in steps
            .iter()
            .filter(|step| step.uses_action("actions/cache"))
        {
            findings.push(common::finding(
                workflow,
                &CACHE_MISUSE,
                step.key("uses"),
                step.location.clone(),
                "setup action has a built-in `cache` input for this",
            )?);
        }
    }
    Ok(findings)
}

pub fn duplicated_steps(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        let mut seen: Vec<Mapping> = Vec::new();
        for step in common::steps(&job) {
            let Some(mapping) = step.value.as_mapping() else {
                continue;
            };
            if mapping.contains_key("parallel") {
                continue;
            }
            let mut body = mapping.clone();
            body.remove("name");
            body.remove("id");
            if seen.contains(&body) {
                findings.push(common::finding(
                    workflow,
                    &DUPLICATED_STEPS,
                    step.anchor(),
                    step.location.clone(),
                    "step repeats an earlier step in the same job",
                )?);
            } else {
                seen.push(body);
            }
        }
    }
    Ok(findings)
}

// ponytail: Fixed ceilings; make them configurable when a project needs other limits.
const MAX_STEPS: usize = 20;
const MAX_SCRIPT_LINES: usize = 20;
const MAX_LOGICAL_OPERATORS: usize = 2;

pub fn complexity(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        let count = job
            .get("steps")
            .and_then(Value::as_sequence)
            .map_or(0, Vec::len);
        if count > MAX_STEPS {
            findings.push(common::finding(
                workflow,
                &COMPLEXITY,
                job.route.clone(),
                job.location(),
                format!("job has {count} steps (limit {MAX_STEPS})"),
            )?);
        }
        let mut conditions = vec![(job.route.with_key("if"), job.location(), job.get("if"))];
        for step in common::steps(&job) {
            let lines = step.run().map_or(0, |script| {
                script
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count()
            });
            if lines > MAX_SCRIPT_LINES {
                findings.push(common::finding(
                    workflow,
                    &COMPLEXITY,
                    step.key("run"),
                    step.location.clone(),
                    format!("script has {lines} lines (limit {MAX_SCRIPT_LINES})"),
                )?);
            }
            conditions.push((step.key("if"), step.location.clone(), step.value.get("if")));
        }
        for (route, location, condition) in conditions {
            let Some(condition) = condition.and_then(Value::as_str) else {
                continue;
            };
            let operators = condition.matches("&&").count() + condition.matches("||").count();
            if operators > MAX_LOGICAL_OPERATORS {
                findings.push(common::finding(
                    workflow,
                    &COMPLEXITY,
                    route,
                    location,
                    format!("condition has {operators} logical operators (limit {MAX_LOGICAL_OPERATORS})"),
                )?);
            }
        }
    }
    Ok(findings)
}

pub fn readability(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    if workflow.value.get("name").is_none() {
        findings.push(common::finding(
            workflow,
            &READABILITY,
            Route::default().with_key("on"),
            "workflow",
            "workflow has no `name`",
        )?);
    }
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            if step.run().is_some() && step.value.get("name").is_none() {
                findings.push(common::finding(
                    workflow,
                    &READABILITY,
                    step.key("run"),
                    step.location.clone(),
                    "`run` step has no `name`",
                )?);
            }
        }
    }
    Ok(findings)
}

const DEPRECATED_COMMANDS: &[&str] = &["::set-output", "::save-state", "::set-env", "::add-path"];

const OPT_OUTS: &[(&str, &str)] = &[
    (
        "ACTIONS_ALLOW_UNSECURE_COMMANDS",
        "re-enables the disabled `set-env` and `add-path` commands",
    ),
    (
        "ACTIONS_ALLOW_USE_UNSECURE_NODE_VERSION",
        "the Node 20 opt-out was removed; update the action to one that runs on Node 24",
    ),
];

pub fn deprecated(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    let mut scopes = vec![(Route::default(), "workflow".to_owned(), &workflow.value)];
    for job in common::jobs(workflow) {
        scopes.push((job.route.clone(), job.location(), job.value));
        for step in common::steps(&job) {
            if let Some(command) = step.run().and_then(|script| {
                DEPRECATED_COMMANDS
                    .iter()
                    .find(|command| script.contains(*command))
            }) {
                findings.push(common::finding(
                    workflow,
                    &DEPRECATED,
                    step.key("run"),
                    step.location.clone(),
                    format!("`{command}` is a deprecated workflow command"),
                )?);
            }
            scopes.push((step.route.clone(), step.location.clone(), step.value));
        }
    }
    for (route, location, scope) in scopes {
        for (variable, message) in OPT_OUTS {
            if scope.get("env").and_then(|env| env.get(variable)).is_some() {
                findings.push(common::finding(
                    workflow,
                    &DEPRECATED,
                    route.with_key("env").with_key(*variable),
                    location.clone(),
                    *message,
                )?);
            }
        }
    }
    Ok(findings)
}
