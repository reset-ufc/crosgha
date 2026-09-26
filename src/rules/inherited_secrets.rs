use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Rule};
use crate::{policy::Severity, workflow::Workflow};

// GitHub defines `secrets: inherit` as passing every available secret to the called workflow:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idsecretsinherit
pub const RULE: Rule = Rule {
    id: "inherited-workflow-secrets",
    tag: "permissions-secrets",
    severity: Severity::Note,
};

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let Some(jobs) = workflow.value.get("jobs").and_then(Value::as_mapping) else {
        return Ok(Vec::new());
    };
    let mut findings = Vec::new();
    for (name, job) in jobs {
        let Some(job_name) = name.as_str() else {
            continue;
        };
        if job.get("uses").and_then(Value::as_str).is_none()
            || job.get("secrets").and_then(Value::as_str) != Some("inherit")
        {
            continue;
        }
        let route = Route::default()
            .with_key("jobs")
            .with_key(job_name.to_owned())
            .with_key("secrets");
        let feature = workflow
            .document
            .query_key_only(&route)
            .map_err(|error| error.to_string())?;
        findings.push(Finding {
            rule: &RULE,
            message: "reusable workflow inherits every available secret",
            location: format!("job `{job_name}`"),
            line: feature.location.point_span.0.0 + 1,
            fix: None,
        });
    }
    Ok(findings)
}
