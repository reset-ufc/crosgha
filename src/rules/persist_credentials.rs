use yaml_serde::{Mapping, Value};
use yamlpatch::{Op, Patch};
use yamlpath::Route;

use super::{ChangeKind, Finding, ProposedFix, Rule};
use crate::{policy::Severity, workflow::Workflow};

// actions/checkout documents the true default and the false opt-out:
// https://github.com/actions/checkout/blob/main/README.md#usage
pub const RULE: Rule = Rule {
    id: "persist-credentials",
    tag: "permissions-secrets",
    severity: Severity::Warning,
};

enum NeededChange {
    AddWith,
    AddInput,
    ReplaceInput,
}

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let Some(jobs) = workflow.value.get("jobs").and_then(Value::as_mapping) else {
        return Ok(Vec::new());
    };
    let mut findings = Vec::new();

    for (name, job) in jobs {
        let Some(job_name) = name.as_str() else {
            continue;
        };
        let Some(steps) = job.get("steps").and_then(Value::as_sequence) else {
            continue;
        };
        for (index, step) in steps.iter().enumerate() {
            // ponytail: Handle literal first-party checkout refs and credential inputs;
            // add expression analysis when their runtime values can be proven.
            let Some(uses) = step.get("uses").and_then(Value::as_str) else {
                continue;
            };
            if !uses.starts_with("actions/checkout@") {
                continue;
            }

            let change = match step.get("with") {
                None => Some(NeededChange::AddWith),
                Some(Value::Mapping(inputs)) => match inputs.get("persist-credentials") {
                    None => Some(NeededChange::AddInput),
                    Some(Value::Bool(true)) => Some(NeededChange::ReplaceInput),
                    Some(Value::String(value)) if value == "true" => {
                        Some(NeededChange::ReplaceInput)
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(change) = change else { continue };

            let step_route = Route::default()
                .with_key("jobs")
                .with_key(job_name.to_owned())
                .with_key("steps")
                .with_key(index);
            let uses_feature = workflow
                .document
                .query_key_only(&step_route.with_key("uses"))
                .map_err(|error| error.to_string())?;
            let fix = propose_fix(step_route, change);
            findings.push(Finding {
                rule: &RULE,
                message: "checkout retains credentials for later steps",
                job: job_name.to_owned(),
                step: index,
                line: uses_feature.location.point_span.0.0 + 1,
                fix: Some(fix),
            });
        }
    }

    Ok(findings)
}

fn propose_fix(step: Route<'static>, change: NeededChange) -> ProposedFix {
    let (patch, changed_route, change) = match change {
        NeededChange::AddWith => {
            let mut inputs = Mapping::new();
            inputs.insert(
                Value::String("persist-credentials".to_owned()),
                Value::Bool(false),
            );
            (
                Patch {
                    route: step.clone(),
                    operation: Op::Add {
                        key: "with".to_owned(),
                        value: Value::Mapping(inputs),
                    },
                },
                step.with_key("with"),
                ChangeKind::Add,
            )
        }
        NeededChange::AddInput => {
            let with = step.with_key("with");
            (
                Patch {
                    route: with.clone(),
                    operation: Op::Add {
                        key: "persist-credentials".to_owned(),
                        value: Value::Bool(false),
                    },
                },
                with.with_key("persist-credentials"),
                ChangeKind::Add,
            )
        }
        NeededChange::ReplaceInput => {
            let input = step.with_key("with").with_key("persist-credentials");
            (
                Patch {
                    route: input.clone(),
                    operation: Op::Replace(Value::Bool(false)),
                },
                input,
                ChangeKind::Replace,
            )
        }
    };
    ProposedFix {
        patch,
        changed_route,
        change,
    }
}
