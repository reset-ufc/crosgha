use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Rule};
use crate::{policy::Severity, workflow::Workflow};

// GitHub documents workflow-level and job-level GITHUB_TOKEN permissions:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#permissions
pub const RULE: Rule = Rule {
    id: "implicit-token-permissions",
    tag: "permissions-secrets",
    severity: Severity::Warning,
};

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    if workflow.value.get("permissions").is_some() {
        return Ok(Vec::new());
    }
    let Some(jobs) = workflow.value.get("jobs").and_then(Value::as_mapping) else {
        return Ok(Vec::new());
    };
    let mut findings = Vec::new();
    for (name, job) in jobs {
        let Some(job_name) = name.as_str() else {
            continue;
        };
        if job.get("permissions").is_some() {
            continue;
        }
        let route = Route::default()
            .with_key("jobs")
            .with_key(job_name.to_owned());
        let feature = workflow
            .document
            .query_key_only(&route)
            .map_err(|error| error.to_string())?;
        findings.push(Finding {
            rule: &RULE,
            message: "job relies on default GITHUB_TOKEN permissions",
            location: format!("job `{job_name}`"),
            line: feature.location.point_span.0.0 + 1,
            fix: None,
        });
    }
    Ok(findings)
}
