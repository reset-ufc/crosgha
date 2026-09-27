use yaml_serde::Value;

use super::{Finding, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GitHub: self-hosted runners should not serve public pull requests.
// https://docs.github.com/en/actions/reference/security/secure-use#hardening-for-self-hosted-runners
// Poutine `pr_runs_on_self_hosted`.
pub const SELF_HOSTED: Rule = Rule {
    id: "self-hosted-runner",
    tag: "self-hosted-runner",
    severity: Severity::Warning,
    persona: Persona::Regular,
    aliases: &[],
};

// StepSecurity secure-repo `harden-runner-integration`: a runtime monitor as each job's first step.
pub const MONITORING: Rule = Rule {
    id: "runtime-monitoring",
    tag: "runtime-monitoring",
    severity: Severity::Note,
    persona: Persona::Auditor,
    aliases: &[],
};

// Khatami et al. (SCAM 2024), KZ-15 "Use fixed version for runs-on argument".
pub const VERSIONING: Rule = Rule {
    id: "runner-versioning",
    tag: "runner-versioning",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    aliases: &[],
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

pub fn self_hosted(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    if !common::has_trigger(workflow, UNTRUSTED_RUNNER_TRIGGERS) {
        return Ok(Vec::new());
    }
    common::jobs(workflow)
        .iter()
        .filter(|job| {
            job.get("runs-on")
                .is_some_and(|runs_on| labels(runs_on).contains(&"self-hosted"))
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
