use crate::{
    fix::{self, FixOutcome},
    policy::Settings,
    rules::{self, Finding},
    workflow::Workflow,
};

pub struct Inspection {
    /// The source the findings' lines point into.
    pub source: String,
    pub findings: Vec<Finding>,
    pub fix: Option<FixOutcome>,
}

pub fn inspect(
    source: String,
    apply_fixes: bool,
    settings: &Settings,
    file: &str,
) -> Result<Inspection, String> {
    let workflow = Workflow::parse(source.clone())?;
    let findings = rules::detect_all(&workflow, settings, file)?;
    let fix = if apply_fixes && !findings.is_empty() {
        Some(fix::apply(workflow, &findings, settings, file)?)
    } else {
        None
    };
    Ok(Inspection {
        source,
        findings,
        fix,
    })
}
