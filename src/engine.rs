use crate::{
    fix::{self, FixOutcome},
    policy::RulePolicy,
    rules::{self, Finding},
    workflow::Workflow,
};

pub struct Inspection {
    pub jobs_start: (usize, usize),
    pub findings: Vec<Finding>,
    pub fix: Option<FixOutcome>,
}

pub fn inspect(
    source: String,
    apply_fixes: bool,
    policy: RulePolicy,
) -> Result<Inspection, String> {
    let workflow = Workflow::parse(source)?;
    let jobs_start = workflow.jobs_start;
    let findings = rules::detect_all(&workflow, policy)?;
    let fix = if apply_fixes && !findings.is_empty() {
        Some(fix::apply(workflow, &findings, policy)?)
    } else {
        None
    };
    Ok(Inspection {
        jobs_start,
        findings,
        fix,
    })
}
