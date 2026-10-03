use yaml_serde::{Mapping, Value};
use yamlpatch::{Op, Patch};
use yamlpath::Route;

use super::{ChangeKind, Finding, ProposedFix, Rule, common, online, repository, runner};
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
// Hosted runner images, action versions, and action inputs that GitHub or the action removed, so the
// job no longer runs. Sources are next to each list below.
pub const REMOVED: Rule = Rule {
    id: "removed-feature",
    summary: "Retired runner images, shut-down action versions, and removed action inputs must not be used.",
    tag: "deprecated-command/feature",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[],
    configurable: false,
};

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
    if FORK_GUARDS.iter().any(|guard| condition.contains(guard)) {
        return true;
    }
    if ["always()", "failure()", "cancelled()"]
        .iter()
        .any(|status| condition.contains(status))
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
        .any(|script| {
            script
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| TEST_COMMANDS.contains(&word))
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
                let version = versioned_action(workflow, step).map(|(_, version)| version);
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

/// Hosted images and the date GitHub fully retired them:
/// https://github.com/actions/runner-images/issues/6002 (ubuntu-18.04)
/// https://github.blog/changelog/2025-01-15-github-actions-ubuntu-20-runner-image-brownout-dates-and-other-breaking-changes/
/// https://github.blog/changelog/2025-04-15-upcoming-breaking-changes-and-releases-for-github-actions/ (windows-2019)
/// https://github.blog/changelog/2025-07-11-upcoming-changes-to-macos-hosted-runners-macos-latest-migration-and-xcode-support-policy-updates/
// ponytail: a fixed list with no runtime lookup; add each image when GitHub retires it.
const RETIRED_RUNNERS: &[(&str, &str)] = &[
    ("ubuntu-18.04", "2023-04-03"),
    ("ubuntu-20.04", "2025-04-15"),
    ("windows-2019", "2025-06-30"),
    ("macos-13", "2025-12-04"),
];

/// Why an action version no longer runs, from its `major.minor` numbers.
fn shut_down(action: &str, version: &[u64]) -> Option<&'static str> {
    match (action, version) {
        // https://github.blog/changelog/2024-04-16-deprecation-notice-v3-of-the-artifact-actions/
        ("actions/upload-artifact" | "actions/download-artifact", [major, ..]) if *major <= 3 => {
            Some("artifact actions v3 and older stopped working on 2025-01-30; use v4 or later")
        }
        // https://github.com/actions/cache/blob/main/README.md: "upgrade to the recommended
        // versions `v4.2.0` or `v3.4.0`", else "workflow runs ... will fail".
        ("actions/cache", [major, ..]) if *major <= 2 => Some(
            "actions/cache below v3.4.0 fails since the cache service moved; use v4.2.0 or later",
        ),
        ("actions/cache", [3, minor, ..]) if *minor < 4 => Some(
            "actions/cache below v3.4.0 fails since the cache service moved; use v4.2.0 or later",
        ),
        ("actions/cache", [4, minor, ..]) if *minor < 2 => Some(
            "actions/cache below v4.2.0 fails since the cache service moved; use v4.2.0 or later",
        ),
        _ => None,
    }
}

/// setup-java v6 removed the AdoptOpenJDK distributions: "Use `temurin` instead of `adopt` or
/// `adopt-hotspot`, and `semeru` instead of `adopt-openj9`."
/// https://github.com/actions/setup-java/blob/main/README.md
const ADOPT_SUCCESSORS: &[(&str, &str)] = &[
    ("adopt", "temurin"),
    ("adopt-hotspot", "temurin"),
    ("adopt-openj9", "semeru"),
];

/// `owner/repo` (lowercased, as GitHub resolves it) and the `major.minor...` numbers of a step's
/// action, when its ref names a version.
pub(super) fn versioned_action(
    workflow: &Workflow,
    step: &common::Step<'_>,
) -> Option<(String, Vec<u64>)> {
    let action = step.uses().and_then(common::parse_uses)?;
    let version = online::ref_version(workflow, &step.key("uses"), action.git_ref)?;
    let name = format!("{}/{}", action.owner, action.repo).to_ascii_lowercase();
    Some((name, online::numbers(&version)))
}

/// github.com facts: retired hosted-runner images and action versions its services no longer
/// serve. Registered in `HOSTED`, so another `--gh-hostname` skips it.
pub fn removed_hosted(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        // Labels of a runner group or of a self-hosted runner name a machine, not a hosted image.
        let grouped = job
            .get("runs-on")
            .is_some_and(|runs_on| runs_on.get("group").is_some());
        let labels = runner::job_labels(&job);
        let retired = if grouped || labels.contains(&"self-hosted") {
            None
        } else {
            labels
                .iter()
                .find_map(|label| RETIRED_RUNNERS.iter().find(|(name, _)| name == label))
        };
        if let Some((label, date)) = retired {
            findings.push(common::finding(
                workflow,
                &REMOVED,
                job.route.with_key("runs-on"),
                job.location(),
                format!("the `{label}` hosted runner image was retired on {date}"),
            )?);
        }
        for step in common::steps(&job) {
            let Some((name, version)) = versioned_action(workflow, &step) else {
                continue;
            };
            if let Some(reason) = shut_down(&name, &version) {
                findings.push(common::finding(
                    workflow,
                    &REMOVED,
                    step.key("uses"),
                    step.location.clone(),
                    reason,
                )?);
            }
        }
    }
    Ok(findings)
}

/// Majors of GitHub's own JavaScript actions whose latest release declares a retired Node.js
/// runtime (`repository::RUNTIME`), read from `runs.using` in each major's newest `vX.Y.Z` tag on
/// 2026-09-27. A major is the unit because `@vN` follows its newest release.
// ponytail: `actions/*` only and major granularity, so a pin to an older minor of a node24 major
// (setup-go v6.0.0) is missed; per-release data or more owners would close both gaps.
const RETIRED_RUNTIME_MAJORS: &[(&str, &[(u64, &str)])] = &[
    ("actions/add-to-project", &[(0, "node20"), (1, "node20")]),
    (
        "actions/cache",
        &[
            (0, "node12"),
            (1, "node12"),
            (2, "node12"),
            (3, "node16"),
            (4, "node20"),
        ],
    ),
    (
        "actions/checkout",
        &[(2, "node12"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/configure-pages",
        &[
            (0, "node16"),
            (1, "node16"),
            (2, "node16"),
            (3, "node16"),
            (4, "node20"),
            (5, "node20"),
        ],
    ),
    (
        "actions/create-github-app-token",
        &[(1, "node20"), (2, "node20")],
    ),
    ("actions/create-release", &[(1, "node12")]),
    (
        "actions/dependency-review-action",
        &[(1, "node16"), (2, "node16"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/deploy-pages",
        &[(1, "node16"), (2, "node16"), (3, "node20"), (4, "node20")],
    ),
    (
        "actions/download-artifact",
        &[(2, "node12"), (4, "node20"), (5, "node20"), (6, "node20")],
    ),
    (
        "actions/github-script",
        &[
            (1, "node12"),
            (2, "node12"),
            (3, "node12"),
            (4, "node12"),
            (5, "node12"),
            (6, "node16"),
            (7, "node20"),
        ],
    ),
    (
        "actions/labeler",
        &[(2, "node12"), (3, "node12"), (4, "node16"), (5, "node20")],
    ),
    (
        "actions/setup-dotnet",
        &[(1, "node12"), (2, "node16"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/setup-go",
        &[
            (1, "node12"),
            (2, "node12"),
            (3, "node16"),
            (4, "node16"),
            (5, "node20"),
        ],
    ),
    (
        "actions/setup-java",
        &[(1, "node12"), (2, "node12"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/setup-node",
        &[(1, "node12"), (2, "node12"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/setup-python",
        &[
            (1, "node12"),
            (2, "node12"),
            (3, "node16"),
            (4, "node16"),
            (5, "node20"),
        ],
    ),
    ("actions/setup-ruby", &[(1, "node12")]),
    (
        "actions/stale",
        &[
            (1, "node12"),
            (2, "node12"),
            (3, "node12"),
            (4, "node12"),
            (5, "node16"),
            (6, "node16"),
            (7, "node16"),
            (8, "node16"),
            (9, "node20"),
        ],
    ),
    (
        "actions/upload-artifact",
        &[(2, "node12"), (4, "node20"), (5, "node20")],
    ),
    ("actions/upload-release-asset", &[(1, "node12")]),
];

/// Steps that use a version of an `actions/*` action built for a retired Node.js runtime.
pub fn retired_runtime(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some((name, version)) = versioned_action(workflow, &step) else {
                continue;
            };
            let runtime = RETIRED_RUNTIME_MAJORS
                .iter()
                .find(|(action, _)| *action == name)
                .and_then(|(_, majors)| {
                    majors
                        .iter()
                        .find(|(major, _)| Some(major) == version.first())
                });
            if let Some((major, runtime)) = runtime {
                findings.push(common::finding(
                    workflow,
                    &repository::RUNTIME,
                    step.key("uses"),
                    step.location.clone(),
                    format!(
                        "{name} v{major} declares `{runtime}`, which GitHub retired; runners force it onto Node 24, so use a release built for `node24`"
                    ),
                )?);
            }
        }
    }
    Ok(findings)
}

/// Inputs an action's newer major removed, whatever host runs it.
pub fn removed(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some((name, version)) = versioned_action(workflow, &step) else {
                continue;
            };
            let successor = step
                .value
                .get("with")
                .and_then(|with| with.get("distribution"))
                .and_then(Value::as_str)
                .and_then(|distribution| {
                    ADOPT_SUCCESSORS
                        .iter()
                        .find(|(adopt, _)| *adopt == distribution)
                });
            if name == "actions/setup-java"
                && version.first().is_some_and(|major| *major >= 6)
                && let Some((adopt, successor)) = successor
            {
                let input = step.key("with").with_key("distribution");
                let mut finding = common::finding(
                    workflow,
                    &REMOVED,
                    step.key("uses"),
                    step.location.clone(),
                    format!("setup-java v6 removed the `{adopt}` distribution; use `{successor}`"),
                )?;
                finding.fix = Some(ProposedFix {
                    patch: Patch {
                        route: input.clone(),
                        operation: Op::Replace(Value::String((*successor).to_owned())),
                    },
                    changed_route: input,
                    change: ChangeKind::Replace,
                });
                findings.push(finding);
            }
        }
    }
    Ok(findings)
}
