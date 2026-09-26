use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Rule};
use crate::{policy::Severity, workflow::Workflow};

// GitHub recommends full-length commit SHAs for remote actions and reusable workflows:
// https://docs.github.com/en/actions/reference/security/secure-use#using-third-party-actions
// The syntax distinguishes local actions, remote actions, reusable workflows, and Docker images:
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idstepsuses
// OCI registers SHA-256 and SHA-512 digest algorithms:
// https://github.com/opencontainers/image-spec/blob/main/descriptor.md#registered-algorithms
pub const RULE: Rule = Rule {
    id: "mutable-action-reference",
    tag: "supply-chain",
    severity: Severity::Warning,
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
        let job_route = Route::default()
            .with_key("jobs")
            .with_key(job_name.to_owned());

        if let Some(uses) = job.get("uses").and_then(Value::as_str) {
            check_action(
                workflow,
                &mut findings,
                job_route.clone().with_key("uses"),
                format!("job `{job_name}`"),
                uses,
            )?;
        }

        if let Some(container) = job.get("container") {
            let image = container
                .as_str()
                .or_else(|| container.get("image").and_then(Value::as_str));
            if let Some(image) = image {
                let mut route = job_route.clone().with_key("container");
                if container.as_mapping().is_some() {
                    route = route.with_key("image");
                }
                check_image(
                    workflow,
                    &mut findings,
                    route,
                    format!("job `{job_name}`, container"),
                    image,
                )?;
            }
        }

        if let Some(services) = job.get("services").and_then(Value::as_mapping) {
            for (name, service) in services {
                let (Some(service_name), Some(image)) =
                    (name.as_str(), service.get("image").and_then(Value::as_str))
                else {
                    continue;
                };
                check_image(
                    workflow,
                    &mut findings,
                    job_route
                        .clone()
                        .with_key("services")
                        .with_key(service_name.to_owned())
                        .with_key("image"),
                    format!("job `{job_name}`, service `{service_name}`"),
                    image,
                )?;
            }
        }

        if let Some(steps) = job.get("steps").and_then(Value::as_sequence) {
            for (index, step) in steps.iter().enumerate() {
                let Some(uses) = step.get("uses").and_then(Value::as_str) else {
                    continue;
                };
                check_action(
                    workflow,
                    &mut findings,
                    job_route
                        .clone()
                        .with_key("steps")
                        .with_key(index)
                        .with_key("uses"),
                    format!("job `{job_name}`, step {}", index + 1),
                    uses,
                )?;
            }
        }
    }

    Ok(findings)
}

fn check_action(
    workflow: &Workflow,
    findings: &mut Vec<Finding>,
    route: Route<'static>,
    location: String,
    reference: &str,
) -> Result<(), String> {
    if reference.starts_with("./") || reference.starts_with("$/") || reference.contains("${{") {
        return Ok(());
    }
    if let Some(image) = reference.strip_prefix("docker://") {
        return check_image(workflow, findings, route, location, image);
    }
    // ponytail: Check literal full-length SHA syntax; resolve commit ownership
    // through the hosting provider when reference identity checks are added.
    let pinned = reference
        .rsplit_once('@')
        .is_some_and(|(repository, revision)| !repository.is_empty() && full_hex(revision, 40));
    if !pinned {
        push_finding(
            workflow,
            findings,
            route,
            location,
            "remote action or workflow is not pinned to a full commit SHA",
        )?;
    }
    Ok(())
}

fn check_image(
    workflow: &Workflow,
    findings: &mut Vec<Finding>,
    route: Route<'static>,
    location: String,
    image: &str,
) -> Result<(), String> {
    if image.contains("${{") {
        return Ok(());
    }
    // ponytail: Check literal SHA-256/SHA-512 digests; add an OCI reference
    // parser when other digest algorithms or dynamic image values are needed.
    let pinned = image.rsplit_once('@').is_some_and(|(name, digest)| {
        !name.is_empty()
            && digest.split_once(':').is_some_and(|(algorithm, encoded)| {
                let length = match algorithm {
                    "sha256" => 64,
                    "sha512" => 128,
                    _ => return false,
                };
                full_hex(encoded, length)
            })
    });
    if !pinned {
        push_finding(
            workflow,
            findings,
            route,
            location,
            "container image is not pinned to a digest",
        )?;
    }
    Ok(())
}

fn full_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn push_finding(
    workflow: &Workflow,
    findings: &mut Vec<Finding>,
    route: Route<'static>,
    location: String,
    message: &'static str,
) -> Result<(), String> {
    let feature = workflow
        .document
        .query_key_only(&route)
        .map_err(|error| error.to_string())?;
    findings.push(Finding {
        rule: &RULE,
        message,
        location,
        line: feature.location.point_span.0.0 + 1,
        fix: None,
    });
    Ok(())
}
