use yaml_serde::{Mapping, Value};
use yamlpatch::{Op, Patch};
use yamlpath::Route;

use super::{ChangeKind, Finding, ProposedFix, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// actions/checkout documents the true default and the false opt-out:
// https://github.com/actions/checkout/blob/main/README.md#usage
pub const RULE: Rule = Rule {
    id: "persist-credentials",
    tag: "persist-credentials",
    severity: Severity::Warning,
    persona: Persona::Regular,
    aliases: &["artipacked"],
};

enum NeededChange {
    AddWith,
    AddInput,
    ReplaceInput,
}

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            // ponytail: Handle literal first-party checkout refs and credential inputs;
            // add expression analysis when their runtime values can be proven.
            if !step
                .uses()
                .is_some_and(|uses| uses.starts_with("actions/checkout@"))
            {
                continue;
            }
            let change = match step.value.get("with") {
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
            let mut finding = common::finding(
                workflow,
                &RULE,
                step.key("uses"),
                step.location.clone(),
                "checkout retains credentials for later steps",
            )?;
            finding.fix = Some(propose_fix(step.route.clone(), change));
            findings.push(finding);
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
