use crate::{
    fix::{self, FixOutcome},
    rules::{self, Finding},
    workflow::Workflow,
};

pub struct Inspection {
    pub jobs_start: (usize, usize),
    pub findings: Vec<Finding>,
    pub fix: Option<FixOutcome>,
}

pub fn inspect(source: String, apply_fixes: bool) -> Result<Inspection, String> {
    let workflow = Workflow::parse(source)?;
    let jobs_start = workflow.jobs_start;
    let findings = rules::detect_all(&workflow)?;
    let fix = if apply_fixes && !findings.is_empty() {
        Some(fix::apply(workflow, &findings)?)
    } else {
        None
    };
    Ok(Inspection {
        jobs_start,
        findings,
        fix,
    })
}
