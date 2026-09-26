use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Rule};
use crate::{policy::Severity, workflow::Workflow};

// GitHub documents job timeouts and the platform default here:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idtimeout-minutes
pub const RULE: Rule = Rule {
    id: "missing-job-timeout",
    tag: "maintainability",
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
        // ponytail: Check presence on jobs with a runner; add a configured
        // numeric ceiling when the policy needs to enforce timeout values.
        if job.get("runs-on").is_none() || job.get("timeout-minutes").is_some() {
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
            message: "executable job has no timeout-minutes",
            location: format!("job `{job_name}`"),
            line: feature.location.point_span.0.0 + 1,
            fix: None,
        });
    }
    Ok(findings)
}
