use super::{Finding, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GitHub documents job timeouts and the platform default here:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idtimeout-minutes
pub const RULE: Rule = Rule {
    id: "missing-timeout",
    tag: "missing-timeout",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    aliases: &[],
};

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    // ponytail: Check presence on jobs with a runner; add a configured
    // numeric ceiling when the policy needs to enforce timeout values.
    common::jobs(workflow)
        .iter()
        .filter(|job| job.get("runs-on").is_some() && job.get("timeout-minutes").is_none())
        .map(|job| {
            common::finding(
                workflow,
                &RULE,
                job.route.clone(),
                job.location(),
                "executable job has no timeout-minutes",
            )
        })
        .collect()
}
