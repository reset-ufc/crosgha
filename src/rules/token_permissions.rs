use super::{Finding, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GitHub documents workflow-level and job-level GITHUB_TOKEN permissions:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#permissions
pub const RULE: Rule = Rule {
    id: "missing-permissions-block",
    summary: "Jobs must declare GITHUB_TOKEN permissions instead of relying on defaults.",
    tag: "missing-permissions-block",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
    relational: false,
};

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    if workflow.value.get("permissions").is_some() {
        return Ok(Vec::new());
    }
    common::jobs(workflow)
        .iter()
        .filter(|job| job.get("permissions").is_none())
        .map(|job| {
            common::finding(
                workflow,
                &RULE,
                job.route.clone(),
                job.location(),
                "job relies on default GITHUB_TOKEN permissions",
            )
        })
        .collect()
}
