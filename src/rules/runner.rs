use yaml_serde::Value;

use super::{Finding, Options, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GitHub: self-hosted runners should not serve public pull requests.
// https://docs.github.com/en/actions/reference/security/secure-use#hardening-for-self-hosted-runners
// Poutine `pr_runs_on_self_hosted`.
pub const SELF_HOSTED: Rule = Rule {
    id: "self-hosted-runner",
    summary: "Self-hosted runners must not run jobs from pull requests or issues.",
    tag: "self-hosted-runner",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "self-hosted-runner")],
    configurable: true,
};

// StepSecurity secure-repo `harden-runner-integration`: a runtime monitor as each job's first step.
pub const MONITORING: Rule = Rule {
    id: "runtime-monitoring",
    summary: "Jobs should start with a runtime monitor such as harden-runner.",
    tag: "runtime-monitoring",
    severity: Severity::Note,
    persona: Persona::Auditor,
    equivalents: &[],
    configurable: false,
};

// Khatami et al. (SCAM 2024), KZ-15 "Use fixed version for runs-on argument".
pub const VERSIONING: Rule = Rule {
    id: "runner-versioning",
    summary: "`runs-on` should name a fixed image version, not `-latest`.",
    tag: "runner-versioning",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
};

const UNTRUSTED_RUNNER_TRIGGERS: &[&str] = &[
    "pull_request",
    "pull_request_target",
    "pull_request_review",
    "pull_request_review_comment",
    "issues",
    "issue_comment",
    "discussion",
    "discussion_comment",
];

/// Literal runner labels from the string, list, and `labels` mapping forms of `runs-on`.
fn labels(runs_on: &Value) -> Vec<&str> {
    let labels = match runs_on {
        Value::Mapping(_) => runs_on.get("labels").unwrap_or(&Value::Null),
        other => other,
    };
    match labels {
        Value::String(label) => vec![label.as_str()],
        Value::Sequence(items) => items.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
    .into_iter()
    .filter(|label| !label.contains("${{"))
    .collect()
}

/// `rules.self-hosted-runner.config.labels`: custom labels of self-hosted runners, with `*`
/// wildcards, the list actionlint keeps in `self-hosted-runner.labels`.
fn self_hosted_labels(options: Options<'_>) -> Vec<&str> {
    options
        .and_then(|options| options.get("labels"))
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .chain(["self-hosted"])
        .collect()
}

/// `*` matches any run of characters; everything else matches literally.
fn glob(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => {
            text.starts_with(head)
                && (0..=text.len() - head.len()).any(|skip| glob(tail, &text[head.len() + skip..]))
        }
    }
}

pub fn self_hosted(workflow: &Workflow, options: Options) -> Result<Vec<Finding>, String> {
    if !common::has_trigger(workflow, UNTRUSTED_RUNNER_TRIGGERS) {
        return Ok(Vec::new());
    }
    let known = self_hosted_labels(options);
    common::jobs(workflow)
        .iter()
        .filter(|job| {
            job.get("runs-on").is_some_and(|runs_on| {
                labels(runs_on)
                    .iter()
                    .any(|label| known.iter().any(|pattern| glob(pattern, label)))
            })
        })
        .map(|job| {
            common::finding(
                workflow,
                &SELF_HOSTED,
                job.route.with_key("runs-on"),
                job.location(),
                "self-hosted runner accepts jobs from pull requests or issues",
            )
        })
        .collect()
}

pub fn monitoring(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        let steps = common::steps(&job);
        let Some(first) = steps
            .iter()
            .find(|step| step.value.get("parallel").is_none())
        else {
            continue;
        };
        if !first.uses_action("step-security/harden-runner") {
            findings.push(common::finding(
                workflow,
                &MONITORING,
                job.route.clone(),
                job.location(),
                "job has no runtime monitor as its first step",
            )?);
        }
    }
    Ok(findings)
}

pub fn versioning(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    common::jobs(workflow)
        .iter()
        .filter(|job| {
            job.get("runs-on").is_some_and(|runs_on| {
                labels(runs_on)
                    .iter()
                    .any(|label| label.ends_with("-latest"))
            })
        })
        .map(|job| {
            common::finding(
                workflow,
                &VERSIONING,
                job.route.with_key("runs-on"),
                job.location(),
                "`-latest` runner label changes image without notice",
            )
        })
        .collect()
}
