use yaml_serde::{Mapping, Value};
use yamlpatch::{Op, Patch};

use super::{ChangeKind, Finding, ProposedFix, Rule, common, maintainability};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// actions/checkout documents the true default and the false opt-out, and that the persisted token
// "enables your scripts to run authenticated git commands":
// https://github.com/actions/checkout/blob/main/README.md#usage
pub const RULE: Rule = Rule {
    id: "persist-credentials",
    summary: "actions/checkout must not leave the token on disk for later steps; there is no automatic fix when a later step pushes with it, `true` is explicit, or the checkout predates v2.",
    tag: "persist-credentials",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "artipacked")],
    configurable: false,
};

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        let steps = common::steps(&job);
        for (index, step) in steps.iter().enumerate() {
            // ponytail: Handle literal first-party checkout refs and credential inputs;
            // add expression analysis when their runtime values can be proven.
            if !step.uses_action("actions/checkout") {
                continue;
            }
            let explicit = match step.value.get("with") {
                None => false,
                Some(Value::Mapping(inputs)) => match inputs.get("persist-credentials") {
                    None => false,
                    Some(Value::Bool(true)) => true,
                    Some(Value::String(value)) if value == "true" => true,
                    _ => continue,
                },
                _ => continue,
            };
            let unfixable = if predates_input(workflow, step) {
                Some("actions/checkout before v2 has no `persist-credentials` input; upgrade it")
            } else if explicit {
                Some(
                    "`persist-credentials: true` is explicit; set it to false if no step needs the token",
                )
            } else if steps[index + 1..].iter().any(pushes) {
                Some("a later step pushes with the persisted token; give that step its own token")
            } else {
                None
            };
            let message = match unfixable {
                None => "checkout retains credentials for later steps".to_owned(),
                Some(reason) => {
                    format!("checkout retains credentials for later steps; not fixed: {reason}")
                }
            };
            let mut finding = common::finding(
                workflow,
                &RULE,
                step.key("uses"),
                step.location.clone(),
                message,
            )?;
            if unfixable.is_none() {
                finding.fix = Some(propose_fix(step));
            }
            findings.push(finding);
        }
    }
    Ok(findings)
}

/// v1's action.yml declares no `persist-credentials` input; v2 added it:
/// https://github.com/actions/checkout/blob/v1/action.yml
/// https://github.com/actions/checkout/blob/v2/action.yml
fn predates_input(workflow: &Workflow, step: &common::Step<'_>) -> bool {
    maintainability::versioned_action(workflow, step)
        .is_some_and(|(_, version)| version.first().is_some_and(|major| *major < 2))
}

/// Actions that push with the credentials checkout persisted. git-auto-commit-action: "`persist-credentials`
/// in `actions/checkout` must be set to `true` to push new commits to the repository."
/// https://github.com/stefanzweifel/git-auto-commit-action/blob/92648143fd6aebb695590bfe3fc28f92bcf383e4/README.md
const PUSHING_ACTIONS: &[&str] = &["stefanzweifel/git-auto-commit-action"];

/// git's global options that take their value as the next word:
/// https://git-scm.com/docs/git#_options
const GIT_VALUED_OPTIONS: &[&str] = &["-C", "-c", "--git-dir", "--work-tree", "--namespace"];

/// Whether the step runs `git push` or an action that pushes with the persisted token.
fn pushes(step: &common::Step<'_>) -> bool {
    if PUSHING_ACTIONS
        .iter()
        .any(|action| step.uses_action(action))
    {
        return true;
    }
    let Some(script) = step.run() else {
        return false;
    };
    script
        .replace("\\\n", " ")
        .split(['\n', ';', '|', '&', '(', ')', '`', '{', '}'])
        .any(|command| {
            let mut words = command.split_whitespace().skip_while(|word| {
                // `sudo`, `env`, and their options or variable assignments precede the program.
                matches!(
                    *word,
                    "sudo" | "env" | "exec" | "if" | "then" | "else" | "elif" | "do" | "!"
                ) || word.starts_with('-')
                    || word.contains('=')
            });
            if words
                .next()
                .is_none_or(|program| program.rsplit('/').next() != Some("git"))
            {
                return false;
            }
            while let Some(word) = words.next() {
                if GIT_VALUED_OPTIONS.contains(&word) {
                    words.next();
                } else if !word.starts_with('-') {
                    return word == "push";
                }
            }
            false
        })
}

fn propose_fix(step: &common::Step<'_>) -> ProposedFix {
    let (patch, changed_route) = match step.value.get("with") {
        None => {
            let mut inputs = Mapping::new();
            inputs.insert(
                Value::String("persist-credentials".to_owned()),
                Value::Bool(false),
            );
            (
                Patch {
                    route: step.route.clone(),
                    operation: Op::Add {
                        key: "with".to_owned(),
                        value: Value::Mapping(inputs),
                    },
                },
                step.key("with"),
            )
        }
        Some(_) => {
            let with = step.key("with");
            (
                Patch {
                    route: with.clone(),
                    operation: Op::Add {
                        key: "persist-credentials".to_owned(),
                        value: Value::Bool(false),
                    },
                },
                with.with_key("persist-credentials"),
            )
        }
    };
    ProposedFix {
        patch,
        changed_route,
        change: ChangeKind::Add,
    }
}
