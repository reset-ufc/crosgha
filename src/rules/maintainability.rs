use yaml_serde::{Mapping, Value};
use yamlpath::Route;

use super::{
    Finding, Rule, common, correctness,
    expression::{self, Operator},
    shell,
};
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
    relational: false,
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
    relational: false,
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
    relational: false,
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
    relational: false,
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
    relational: false,
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
    relational: false,
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
    relational: true,
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
    relational: false,
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
    relational: false,
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
/// Context paths (and their prefixes, as in `github.repository_owner`) that keep a job in one
/// repository when compared with `==`; `!=` would run it only in forks.
const FORK_GUARDS: &[&str] = &[
    "github.repository",
    "github.event.repository.full_name",
    "github.event.repository.owner",
];
const FORK_FLAG: &str = "github.event.repository.fork";

/// Whether `condition` runs the job only in its own repository. Context names are
/// case-insensitive: https://docs.github.com/en/actions/reference/workflows-and-actions/contexts
fn guards_fork(condition: &str) -> bool {
    let text = expression::dotted(&condition.to_ascii_lowercase());
    expression::context_paths(&text)
        .into_iter()
        .any(|(start, end)| {
            let path = &text[start..end];
            let test = expression::test(&text, start, end);
            if path == FORK_FLAG {
                let boolean = |value: &str| match value {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                };
                let runs_in_forks = match test.operator {
                    Operator::Truthy => Some(true),
                    Operator::Equals(value) => boolean(value),
                    Operator::Differs(value) => boolean(value).map(|value| !value),
                };
                return runs_in_forks == Some(test.negated);
            }
            FORK_GUARDS.iter().any(|guard| path.starts_with(guard))
                && match test.operator {
                    Operator::Equals(_) => !test.negated,
                    Operator::Differs(_) => test.negated,
                    Operator::Truthy => false,
                }
        })
}

/// "If a job fails or is skipped, all jobs that need it are skipped unless the jobs use a
/// conditional expression that causes the job to continue."
/// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idneeds
/// So a job is guarded by its own `if`, or by a guarded job it needs, unless a status function
/// lets it run after that job is skipped.
fn fork_guarded(
    jobs: &[common::Job<'_>],
    job: &common::Job<'_>,
    visited: &mut Vec<String>,
) -> bool {
    if visited.iter().any(|name| name == job.name) {
        return false;
    }
    visited.push(job.name.to_owned());
    let condition = job.get("if").and_then(Value::as_str).unwrap_or("");
    if guards_fork(condition) {
        return true;
    }
    // A status function replaces the implicit `success()`; `success() || ...` still runs when the
    // needed job was skipped.
    let lowered = condition.to_ascii_lowercase();
    if ["always()", "failure()", "cancelled()"]
        .iter()
        .any(|status| lowered.contains(status))
        || lowered.contains("success()") && lowered.contains("||")
    {
        return false;
    }
    let needs = match job.get("needs") {
        Some(Value::String(need)) => vec![need.as_str()],
        Some(Value::Sequence(needs)) => needs.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    needs.iter().any(|need| {
        jobs.iter()
            .find(|other| other.name == *need)
            .is_some_and(|other| fork_guarded(jobs, other, visited))
    })
}

pub fn fork_guard(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    if !common::has_trigger(workflow, &["schedule"]) {
        return Ok(Vec::new());
    }
    let jobs = common::jobs(workflow);
    jobs.iter()
        .filter(|job| !fork_guarded(&jobs, job, &mut Vec::new()))
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

/// Commands that run a test suite. Linters, link checkers, and builds do not depend on the OS or
/// toolchain version the way tests do, so a workflow without tests needs no matrix.
// ponytail: builds that run tests implicitly (`mvn package`, `gradle build`) are not recognized;
// match build-tool goals too if that gap shows up in labeled data.
const TEST_COMMANDS: &[&str] = &[
    "test", "tests", "pytest", "ctest", "tox", "nox", "unittest", "jest", "vitest", "rspec",
    "phpunit",
];

fn runs_tests(job: &common::Job<'_>) -> bool {
    common::steps(job)
        .iter()
        .filter_map(|step| step.run())
        .flat_map(shell::commands)
        .any(|command| {
            let Some(program) = shell::program(&command.words) else {
                return false;
            };
            // `test` and `[` are the shell's condition builtins, and installing a test tool does
            // not run it.
            let installs = program.name == "install"
                || program.args.contains(&"install")
                || matches!(program.name, "npm" | "pnpm" | "yarn" | "bun")
                    && matches!(program.args.first(), Some(&("add" | "i")));
            if installs || matches!(program.name, "test" | "[") {
                return false;
            }
            std::iter::once(program.path)
                .chain(program.args.iter().copied())
                .any(|word| {
                    word.split(|c: char| !c.is_ascii_alphanumeric())
                        .any(|word| TEST_COMMANDS.contains(&word))
                })
        })
}

pub fn matrix(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let jobs = common::jobs(workflow);
    let runs = jobs
        .iter()
        .any(|job| job.get("runs-on").is_some() && runs_tests(job));
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

/// Setup actions, the first release whose `cache` input exists (read from `action.yml` at each tag),
/// and the directories that input restores: the package managers' default locations the action
/// resolves (setup-java `src/cache.ts`; setup-go `go env GOMODCACHE GOCACHE`; setup-node
/// `npm config get cache`, `yarn cache dir`, `pnpm store path`; setup-python pip, poetry, pipenv;
/// setup-dotnet NuGet global-packages).
const SETUP_CACHES: &[(&str, [u64; 2], &[&str])] = &[
    (
        "actions/setup-node",
        [2, 2],
        &[
            "~/.npm",
            "~/.cache/yarn",
            "~/.yarn/berry/cache",
            "~/.local/share/pnpm/store",
            "~/.pnpm-store",
        ],
    ),
    (
        "actions/setup-python",
        [2, 3],
        &[
            "~/.cache/pip",
            "~/.cache/pypoetry",
            "~/.local/share/virtualenvs",
        ],
    ),
    (
        "actions/setup-java",
        [2, 3],
        &[
            "~/.m2/repository",
            "~/.m2/wrapper",
            "~/.gradle/caches",
            "~/.gradle/wrapper",
            "~/.ivy2/cache",
            "~/.sbt",
            "~/.cache/coursier",
        ],
    ),
    (
        "actions/setup-go",
        [3, 2],
        &["~/go/pkg/mod", "~/.cache/go-build"],
    ),
    ("actions/setup-dotnet", [3, 2], &["~/.nuget/packages"]),
];

/// Whether `path` is, or sits inside or around, one of `directories`.
fn overlaps(path: &str, directories: &[&str]) -> bool {
    let path = path.trim_end_matches('/');
    let path = path
        .strip_prefix("/home/runner")
        .map_or(path.to_owned(), |rest| format!("~{rest}"));
    directories.iter().any(|directory| {
        path == *directory
            || path.starts_with(&format!("{directory}/"))
            || directory.starts_with(&format!("{path}/"))
    })
}

pub fn cache_misuse(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        let steps = common::steps(&job);
        // A setup step pinned to a SHA without a version comment is assumed to be recent.
        let covered: Vec<&str> = steps
            .iter()
            .flat_map(|step| {
                let version = common::versioned_action(workflow, step).map(|(_, version)| version);
                SETUP_CACHES
                    .iter()
                    .filter(move |(setup, since, _)| {
                        step.uses_action(setup)
                            && version
                                .as_ref()
                                .is_none_or(|version| version.as_slice() >= since.as_slice())
                    })
                    .flat_map(|(_, _, directories)| directories.iter().copied())
            })
            .collect();
        for step in steps
            .iter()
            .filter(|step| step.uses_action("actions/cache"))
        {
            let paths: Vec<&str> = step
                .value
                .get("with")
                .and_then(|with| with.get("path"))
                .and_then(Value::as_str)
                .into_iter()
                .flat_map(str::lines)
                .map(str::trim)
                .filter(|path| !path.is_empty() && !path.starts_with('!'))
                .collect();
            // Only a cache the setup action can restore completely is redundant.
            if !paths.is_empty() && paths.iter().all(|path| overlaps(path, &covered)) {
                findings.push(common::finding(
                    workflow,
                    &CACHE_MISUSE,
                    step.key("uses"),
                    step.location.clone(),
                    "setup action has a built-in `cache` input for this",
                )?);
            }
        }
    }
    Ok(findings)
}

/// Step `id`s a job reads as `steps.<id>`, in `${{ }}` expressions or bare `if` conditions. Other
/// text without `${{` is not evaluated, so a `steps.<id>` there reads nothing.
fn read_step_ids(job: &common::Job<'_>) -> Vec<String> {
    let mut expressions = Vec::new();
    common::strings(job.value, &job.route, &mut |_, text| {
        expressions.extend(expression::expressions(text));
    });
    let conditions = std::iter::once(job.value)
        .chain(common::steps(job).into_iter().map(|step| step.value))
        .filter_map(|value| value.get("if").and_then(Value::as_str))
        .filter(|condition| !condition.contains("${{"));
    expressions.extend(conditions);
    expressions
        .into_iter()
        .flat_map(|expression| correctness::context_names(expression, "steps"))
        .map(|id| id.to_ascii_lowercase())
        .collect()
}

pub fn duplicated_steps(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        let read = read_step_ids(&job);
        let mut seen: Vec<Mapping> = Vec::new();
        for step in common::steps(&job) {
            let Some(mapping) = step.value.as_mapping() else {
                continue;
            };
            if mapping.contains_key("parallel") {
                continue;
            }
            // A step whose outputs a later step reads is a measurement, not a repeat.
            let output_read = step
                .value
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| read.contains(&id.to_ascii_lowercase()));
            let mut body = mapping.clone();
            body.remove("name");
            body.remove("id");
            if !output_read && seen.contains(&body) {
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
