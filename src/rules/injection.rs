use std::collections::BTreeSet;

use yaml_serde::Value;
use yamlpath::Route;

use super::{
    Finding, Rule,
    common::{self, EXTERNAL_TRIGGERS, PRIVILEGED_TRIGGERS},
    expression::{self, Operator},
    shell,
};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GitHub: untrusted context values expanded into scripts are a script injection risk.
// https://docs.github.com/en/actions/concepts/security/script-injections
pub const SCRIPT_INJECTION: Rule = Rule {
    id: "script-injection",
    summary: "Untrusted event data must not be expanded into scripts with `${{ }}`.",
    tag: "script-injection",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[
        ("zizmor", "template-injection"),
        ("actionlint", "expression"),
    ],
    configurable: false,
    relational: false,
};

// TaintAWI (arXiv 2605.07135) Prompt-to-Agent: untrusted event data reaches an agent prompt.
pub const PROMPT_INJECTION: Rule = Rule {
    id: "llm-prompt-injection",
    summary: "AI agent actions must not receive untrusted event data or run on events outsiders trigger.",
    tag: "llm-prompt-injection",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[],
    configurable: false,
    relational: false,
};

// GitHub Security Lab "pwn request": privileged triggers that check out untrusted code.
// https://securitylab.github.com/resources/github-actions-preventing-pwn-requests/
pub const PWN_REQUEST: Rule = Rule {
    id: "pwn-request",
    summary: "Privileged triggers must not check out or run pull request code.",
    tag: "pwn-request",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[],
    configurable: false,
    relational: false,
};

// zizmor `dangerous-triggers`: triggers that run with base-repository privileges.
pub const DANGEROUS_TRIGGER: Rule = Rule {
    id: "dangerous-trigger",
    summary: "pull_request_target and workflow_run run with base repository privileges.",
    tag: "dangerous-trigger",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    equivalents: &[("zizmor", "dangerous-triggers")],
    configurable: false,
    relational: false,
};

// actionlint `if-cond` constant conditions, zizmor `unsound-condition` and `bot-conditions`.
// GitHub evaluates `if` as an expression only when `${{ }}` wraps the whole value:
// https://docs.github.com/en/actions/reference/workflows-and-actions/expressions
pub const UNSAFE_IF: Rule = Rule {
    id: "unsafe-if-condition",
    summary: "Conditions must not be constant, always true, or based on spoofable bot names.",
    tag: "unsafe-if-condition",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[
        ("zizmor", "bot-conditions"),
        ("zizmor", "unsound-condition"),
        ("actionlint", "if-cond"),
    ],
    configurable: false,
    relational: false,
};

// CodeQL `actions/artifact-poisoning`: artifacts from another run extracted into the workspace.
pub const ARTIFACT_POISONING: Rule = Rule {
    id: "artifact-poisoning",
    summary: "Artifacts from other runs must not be extracted into the workspace.",
    tag: "artifact-poisoning",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[],
    configurable: false,
    relational: false,
};

// GitHub dependency caching docs (write-capable `cache-mode` on low-trust triggers) and
// zizmor `cache-poisoning` (caches restored into release builds).
// https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#bypassing-the-default-untrusted-trigger-cache-restriction
pub const CACHE_POISONING: Rule = Rule {
    id: "cache-poisoning",
    summary: "Caches must not be writable from low-trust triggers or restored into release builds.",
    tag: "cache-poisoning",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "cache-poisoning")],
    configurable: false,
    relational: false,
};

// zizmor `obfuscation`: paths and expressions written to hide what they do.
pub const OBFUSCATION: Rule = Rule {
    id: "obfuscation",
    summary: "`uses` paths and expressions must not hide what they reference.",
    tag: "obfuscation",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    equivalents: &[("zizmor", "obfuscation")],
    configurable: false,
    relational: false,
};

pub fn script_injection(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let mut sinks = Vec::new();
            if let Some(script) = step.run() {
                sinks.push((step.key("run"), script));
            }
            if step.uses_action("actions/github-script")
                && let Some(script) = step.input_str("script")
            {
                sinks.push((step.key("with").with_key("script"), script));
            }
            for (route, script) in sinks {
                if let Some(context) = expression::untrusted_in_text(script).first() {
                    findings.push(common::finding(
                        workflow,
                        &SCRIPT_INJECTION,
                        route,
                        step.location.clone(),
                        format!("untrusted `{context}` expands into a script"),
                    )?);
                }
            }
        }
    }
    Ok(findings)
}

// ponytail: Known agent actions only; add a marketplace-category lookup when
// new agent actions appear faster than this list is maintained.
const AGENT_ACTIONS: &[&str] = &[
    "anthropics/claude-code-action",
    "anthropics/claude-code-base-action",
    "openai/codex-action",
    "google-github-actions/run-gemini-cli",
    "actions/ai-inference",
];

pub fn prompt_injection(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let external = common::has_trigger(workflow, EXTERNAL_TRIGGERS);
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            if !AGENT_ACTIONS.iter().any(|action| step.uses_action(action)) {
                continue;
            }
            let mut tainted = false;
            if let Some(inputs) = step.value.get("with") {
                common::strings(inputs, &Route::default(), &mut |_, text| {
                    tainted |= !expression::untrusted_in_text(text).is_empty();
                });
            }
            if external || tainted {
                findings.push(common::finding(
                    workflow,
                    &PROMPT_INJECTION,
                    step.key("uses"),
                    step.location.clone(),
                    if tainted {
                        "untrusted event data reaches an agent prompt"
                    } else {
                        "agent runs on an event that outsiders can trigger"
                    },
                )?);
            }
        }
    }
    Ok(findings)
}

/// References to the pull request's code: its head (in the fork), the merge commit GitHub
/// builds from it, or the run's head for `workflow_run`.
const HEAD_REFS: &[&str] = &[
    "github.event.pull_request.head.",
    "github.event.pull_request.merge_commit_sha",
    "github.head_ref",
    "refs/pull/",
    "github.event.workflow_run.head_",
];

/// Whether a step checks out or fetches the pull request's code, which the author controls.
/// Any ref of the fork repository counts, so `repository` alone is enough. Context paths are
/// compared as `untrusted_contexts` reads them: lowercased, with index syntax made dotted.
fn checks_out_head(step: &common::Step) -> bool {
    let mentions_head = |text: &str| {
        let text = expression::dotted(&text.to_ascii_lowercase());
        HEAD_REFS.iter().any(|head| text.contains(head))
    };
    let checkout = step
        .uses()
        .is_some_and(|uses| uses.to_ascii_lowercase().starts_with("actions/checkout@"))
        && ["ref", "repository"]
            .iter()
            .any(|input| step.input_str(input).is_some_and(mentions_head));
    let fetch = step.run().is_some_and(|script| {
        shell::commands(script)
            .iter()
            .filter_map(|command| shell::program(&command.words))
            .any(|program| {
                let gh_checkout =
                    program.name == "gh" && program.args.starts_with(&["pr", "checkout"]);
                let git_fetch =
                    shell::git_subcommand(&program).is_some_and(|(subcommand, args)| {
                        let args = args.join(" ");
                        ["fetch", "pull", "clone", "checkout", "switch"].contains(&subcommand)
                            && (args.contains("pull/") || mentions_head(&args))
                    });
                gh_checkout || git_fetch
            })
    });
    checkout || fetch
}

pub fn pwn_request(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    if !common::has_trigger(workflow, PRIVILEGED_TRIGGERS) {
        return Ok(Vec::new());
    }
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            if checks_out_head(&step) {
                findings.push(common::finding(
                    workflow,
                    &PWN_REQUEST,
                    step.anchor(),
                    step.location.clone(),
                    "privileged trigger checks out untrusted pull request code",
                )?);
            }
        }
    }
    Ok(findings)
}

pub fn dangerous_trigger(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    common::triggers(workflow)
        .into_iter()
        .filter(|trigger| PRIVILEGED_TRIGGERS.contains(&trigger.name))
        .map(|trigger| {
            common::finding(
                workflow,
                &DANGEROUS_TRIGGER,
                trigger.route,
                "workflow",
                format!("`{}` runs with base repository privileges", trigger.name),
            )
        })
        .collect()
}

pub fn unsafe_if(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut conditions = Vec::new();
    for job in common::jobs(workflow) {
        if let Some(condition) = job.get("if") {
            conditions.push((job.route.with_key("if"), job.location(), condition));
        }
        for step in common::steps(&job) {
            if let Some(condition) = step.value.get("if") {
                conditions.push((step.key("if"), step.location.clone(), condition));
            }
        }
    }
    let mut findings = Vec::new();
    for (route, location, condition) in conditions {
        let problem = match condition {
            Value::Bool(value) => Some((format!("condition is always {value}"), None)),
            Value::String(text) if text.contains("${{") && !single_expression(text) => Some((
                "text around `${{ }}` makes the condition always true".to_owned(),
                Some("unsound-condition"),
            )),
            Value::String(text) if trusts_bot_actor(text) => Some((
                "actor name checks can be spoofed by a bot-triggered event".to_owned(),
                Some("bot-conditions"),
            )),
            _ => None,
        };
        if let Some((message, concept)) = problem {
            let mut finding = common::finding(workflow, &UNSAFE_IF, route, location, message)?;
            finding.concept = concept;
            findings.push(finding);
        }
    }
    Ok(findings)
}

/// Whether `condition` runs something because the actor is a bot: `==`, `contains`,
/// `startsWith`, or `endsWith` matching `github.actor` or `github.triggering_actor` with a
/// `[bot]` name. A spoofed actor fails a `!=` or negated check, which only skips the work.
fn trusts_bot_actor(condition: &str) -> bool {
    let text = expression::dotted(&condition.to_ascii_lowercase());
    expression::context_paths(&text)
        .into_iter()
        .filter(|&(start, end)| {
            matches!(
                &text[start..end],
                "github.actor" | "github.triggering_actor"
            )
        })
        .any(|(start, end)| {
            let test = expression::test(&text, start, end);
            !test.negated
                && match test.operator {
                    Operator::Equals(operand) => operand.contains("[bot]"),
                    Operator::Differs(_) => false,
                    Operator::Truthy => {
                        expression::enclosing_call(&text, start).is_some_and(|(name, call)| {
                            ["contains", "startswith", "endswith"].contains(&name)
                                && call.contains("[bot]")
                        })
                    }
                }
        })
}

fn single_expression(text: &str) -> bool {
    text.starts_with("${{") && text.ends_with("}}") && text.matches("${{").count() == 1
}

pub fn artifact_poisoning(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    if !common::has_trigger(workflow, &["workflow_run"]) {
        return Ok(Vec::new());
    }
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let cross_run = (step.uses_action("actions/download-artifact")
                && step.input("run-id").is_some())
                || step.uses_action("dawidd6/action-download-artifact");
            let outside_workspace = step.input_str("path").is_some_and(|path| {
                let path = path.replace(' ', "");
                path.starts_with("${{runner.temp}}") || path.starts_with("/tmp")
            });
            if cross_run && !outside_workspace {
                findings.push(common::finding(
                    workflow,
                    &ARTIFACT_POISONING,
                    step.key("uses"),
                    step.location.clone(),
                    "artifact from another run is extracted into the workspace",
                )?);
            }
        }
    }
    Ok(findings)
}

/// Triggers that may write caches in the default branch scope; `pull_request` caches are
/// scoped to the merge ref. `workflow_call` inherits its caller's trust.
const CACHE_TRUSTED_TRIGGERS: &[&str] = &[
    "push",
    "workflow_dispatch",
    "repository_dispatch",
    "delete",
    "registry_package",
    "page_build",
    "schedule",
    "pull_request",
    "workflow_call",
];

pub fn cache_poisoning(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let triggers = common::triggers(workflow);
    let mut findings = Vec::new();

    let low_trust = triggers
        .iter()
        .any(|trigger| !CACHE_TRUSTED_TRIGGERS.contains(&trigger.name));
    if low_trust {
        let mut scopes = vec![(Route::default(), "workflow".to_owned(), &workflow.value)];
        for job in common::jobs(workflow) {
            scopes.push((job.route.clone(), job.location(), job.value));
        }
        for (route, location, scope) in scopes {
            if let Some(mode @ ("write" | "write-only")) =
                scope.get("cache-mode").and_then(Value::as_str)
            {
                findings.push(common::finding(
                    workflow,
                    &CACHE_POISONING,
                    route.with_key("cache-mode"),
                    location,
                    format!("`cache-mode: {mode}` lets a low-trust trigger write caches"),
                )?);
            }
        }
        // Without an explicit cap on the caller, a called workflow may request `write`:
        // https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#cache-access-in-reusable-workflows
        // Only outsider triggers count; maintainer-only ones such as `release` would be noise.
        if common::has_trigger(workflow, EXTERNAL_TRIGGERS)
            && workflow.value.get("cache-mode").is_none()
        {
            for job in common::jobs(workflow) {
                if job.get("uses").is_some() && job.get("cache-mode").is_none() {
                    findings.push(common::finding(
                        workflow,
                        &CACHE_POISONING,
                        job.route.with_key("uses"),
                        job.location(),
                        "called workflow can request cache writes on a low-trust trigger; set `cache-mode: read`",
                    )?);
                }
            }
        }
    }

    // A privileged run shares the base branch's cache scope, so a cache saved after the pull
    // request's code ran is restored by later trusted runs:
    // https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#restrictions-for-accessing-a-cache
    if common::has_trigger(workflow, PRIVILEGED_TRIGGERS) {
        for job in common::jobs(workflow) {
            let steps = common::steps(&job);
            let Some(checkout) = steps.iter().position(checks_out_head) else {
                continue;
            };
            let saves = steps
                .iter()
                .enumerate()
                .filter(|(index, step)| saves_cache(workflow, step, *index > checkout));
            for (_, step) in saves {
                findings.push(common::finding(
                    workflow,
                    &CACHE_POISONING,
                    step.key("uses"),
                    step.location.clone(),
                    "cache saved after pull request code ran on a privileged trigger reaches the base branch",
                )?);
            }
        }
    }

    let release = triggers.iter().any(|trigger| {
        trigger.name == "release"
            || (trigger.name == "push"
                && trigger
                    .config
                    .is_some_and(|config| config.get("tags").is_some()))
    });
    if release {
        for job in common::jobs(workflow) {
            for step in common::steps(&job) {
                if uses_cache(workflow, &step) {
                    findings.push(common::finding(
                        workflow,
                        &CACHE_POISONING,
                        step.key("uses"),
                        step.location.clone(),
                        "release build restores a cache that other runs can write",
                    )?);
                }
            }
        }
    }
    Ok(findings)
}

/// Whether the step writes a cache once the job has run. `actions/cache` and the setup actions
/// save in their `post` step at the end of the job (action.yml of each); `actions/cache/save`
/// saves where it stands, and `actions/cache/restore` never saves.
fn saves_cache(workflow: &Workflow, step: &common::Step, after_checkout: bool) -> bool {
    let name = step
        .uses()
        .map(|uses| uses.split('@').next().unwrap_or("").to_ascii_lowercase());
    match name.as_deref() {
        Some("actions/cache/save") => after_checkout,
        Some("actions/cache/restore") => false,
        // From v5, setup-node also caches when package.json has a `packageManager` field unless
        // `package-manager-cache` is false (action.yml and README at v5.0.0, commit
        // a0853c24544627f65ddf259abe73b1d18a591444). The checked-out pull request controls that
        // file, so any setup-node from v5 may save.
        Some("actions/setup-node") => {
            let before_v5 = common::action_major(workflow, step).is_some_and(|major| major < 5);
            let automatic = !before_v5
                && step
                    .input("package-manager-cache")
                    .and_then(common::input_bool)
                    != Some(false);
            automatic || uses_cache(workflow, step)
        }
        _ => uses_cache(workflow, step),
    }
}

fn uses_cache(workflow: &Workflow, step: &common::Step) -> bool {
    let Some(uses) = step.uses() else {
        return false;
    };
    let name = uses.split('@').next().unwrap_or("").to_ascii_lowercase();
    let cache_input = step.input("cache");
    let disabled = |value: Option<&Value>| {
        value.is_some_and(|value| {
            common::input_bool(value) == Some(false) || value.as_str() == Some("")
        })
    };
    match name.as_str() {
        "actions/cache" | "actions/cache/restore" => true,
        // setup-go's `cache` defaults to true from v4 and to false before (action.yml of each tag).
        "actions/setup-go" => {
            let before_v4 = common::action_major(workflow, step).is_some_and(|major| major < 4);
            if before_v4 {
                cache_input.is_some() && !disabled(cache_input)
            } else {
                !disabled(cache_input)
            }
        }
        // setup-uv's `enable-cache` defaults to false before v5, then to `auto`, which from v10 skips
        // release, tag push, pull_request_target, and workflow_run runs (action.yml of each tag).
        "astral-sh/setup-uv" => match step.input("enable-cache") {
            Some(value @ (Value::Bool(_) | Value::String(_))) if value.as_str() != Some("auto") => {
                common::input_bool(value) == Some(true)
            }
            _ => common::action_major(workflow, step).is_some_and(|major| (5..10).contains(&major)),
        },
        "actions/setup-node"
        | "actions/setup-python"
        | "actions/setup-java"
        | "actions/setup-dotnet" => cache_input.is_some() && !disabled(cache_input),
        _ => false,
    }
}

pub fn obfuscation(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for (route, location, uses) in common::all_uses(workflow) {
        let name = uses.rsplit_once('@').map_or(uses, |(name, _)| name);
        let odd_path = name.contains("//")
            || name
                .split('/')
                .any(|segment| segment == "." || segment == "..")
            || name.ends_with('/');
        let local_or_image = ["./", "$/", "docker://"]
            .iter()
            .any(|prefix| uses.starts_with(prefix));
        if odd_path && !local_or_image {
            findings.push(common::finding(
                workflow,
                &OBFUSCATION,
                route,
                location,
                "`uses` path contains redundant segments",
            )?);
        }
    }

    let mut routes = BTreeSet::new();
    let mut flagged = Vec::new();
    common::strings(&workflow.value, &Route::default(), &mut |route, text| {
        let obfuscated = expression::expressions(text)
            .into_iter()
            .any(|expression| is_string_literal(expression) || expression.contains("github["));
        if obfuscated && routes.insert(format!("{route:?}")) {
            flagged.push(route.clone());
        }
    });
    for route in flagged {
        findings.push(common::finding(
            workflow,
            &OBFUSCATION,
            route,
            "workflow",
            "expression hides a literal or a context path",
        )?);
    }
    Ok(findings)
}

fn is_string_literal(expression: &str) -> bool {
    expression.len() >= 2
        && expression.starts_with('\'')
        && expression.ends_with('\'')
        && !expression[1..expression.len() - 1].contains('\'')
}
