use yaml_serde::Value;

use super::{Finding, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GitHub defines `secrets: inherit` as passing every available secret to the called workflow:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idsecretsinherit
pub const RULE: Rule = Rule {
    id: "secrets-inherit",
    summary: "Reusable workflow calls should pass only the secrets they need.",
    tag: "secrets-inherit",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[("zizmor", "secrets-inherit")],
    configurable: false,
    relational: false,
};

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    common::jobs(workflow)
        .iter()
        .filter(|job| {
            job.get("uses").and_then(Value::as_str).is_some()
                && job.get("secrets").and_then(Value::as_str) == Some("inherit")
        })
        .map(|job| {
            common::finding(
                workflow,
                &RULE,
                job.route.with_key("secrets"),
                job.location(),
                "reusable workflow inherits every available secret",
            )
        })
        .collect()
}
