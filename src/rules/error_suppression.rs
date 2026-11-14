use std::fmt;

use yaml_serde::Value;

use super::{Finding, Rule, common};
use crate::policy::{Persona, Severity};
use crate::workflow::Workflow;

// GASH (Freitas & Rocha, VEM 2024) "Lack of Error Handling: continue-on-error". Actions that pass
// when they find nothing follow ghlint's `FailFastActionsRule.kt` at
// 74115a437cfa161c2fd2d72fc2ebcd83ac514b80, which names one concept per action:
// https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/FailFastActionsRule.kt
pub const RULE: Rule = Rule {
    id: "error-suppression",
    summary: "`continue-on-error: true` hides failures, and so do actions that pass when they find nothing to upload, attach, or publish; create-pull-request and versions before the fail-fast input have no automatic fix.",
    tag: "error-suppression",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[
        ("ghlint", "FailFastUploadArtifact"),
        ("ghlint", "FailFastSoftpropsGhRelease"),
        ("ghlint", "FailFastPublishUnitTestResults"),
        ("ghlint", "FailFastPeterEvansCreatePullRequest"),
    ],
    configurable: false,
    relational: false,
};

/// No ghlint ID covers `continue-on-error`, so its findings name their own part: an empty list
/// would answer to every ghlint ID above.
const CONTINUE_ON_ERROR: &[&str] = &["continue-on-error"];

/// An action that passes when it finds nothing to work on.
struct FailFast {
    /// `owner/repo[/path]` names of the action and its variants.
    names: &'static [&'static str],
    /// The ghlint ID for the action.
    concepts: &'static [&'static str],
    /// What the action does when it finds nothing.
    outcome: &'static str,
    /// An input without which the action looks for nothing, so it cannot come up empty.
    looks_with: Option<&'static str>,
    /// The input that fails the step instead; `None` when the action has none.
    input: Option<Input>,
}

struct Input {
    name: &'static str,
    value: Setting,
    /// The first release whose action.yml declares the input.
    since: &'static [u64],
}

#[derive(Clone, Copy)]
enum Setting {
    Text(&'static str),
    Flag(bool),
}

impl Setting {
    fn to_value(self) -> Value {
        match self {
            Self::Text(text) => Value::String(text.to_owned()),
            Self::Flag(flag) => Value::Bool(flag),
        }
    }
}

impl fmt::Display for Setting {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => formatter.write_str(text),
            Self::Flag(flag) => write!(formatter, "{flag}"),
        }
    }
}

/// Input names and defaults come from each action's action.yml at its latest release, and `since`
/// from the first tag whose action.yml declares the input.
const FAIL_FAST: &[FailFast] = &[
    // `if-no-files-found`, default `warn`:
    // https://github.com/actions/upload-artifact/blob/cf430e030ddbb5b0abf93d22962f4752f3646cd9/action.yml (v7.0.2)
    // v2.1.1 lacks it, v2.1.2 declares it:
    // https://github.com/actions/upload-artifact/blob/c8879bf5aef7bef66f9b82b197f34c4eeeb1731b/action.yml
    FailFast {
        names: &["actions/upload-artifact"],
        concepts: &["FailFastUploadArtifact"],
        outcome: "upload-artifact passes when `path` matches no files",
        looks_with: None,
        input: Some(Input {
            name: "if-no-files-found",
            value: Setting::Text("error"),
            since: &[2, 1, 2],
        }),
    },
    // `fail_on_unmatched_files`, "Defaults to false", and only `files` globs can match nothing:
    // https://github.com/softprops/action-gh-release/blob/8a0cd651e5b2365bd640854a3fa5dfc70e4875f3/action.yml (v3.0.4)
    // v0.1.5 lacks it, v0.1.6 declares it:
    // https://github.com/softprops/action-gh-release/blob/08e53e60c840b6d34ed49473f9ab939eb801dbc4/action.yml
    FailFast {
        names: &["softprops/action-gh-release"],
        concepts: &["FailFastSoftpropsGhRelease"],
        outcome: "action-gh-release passes when a `files` glob matches nothing",
        looks_with: Some("files"),
        input: Some(Input {
            name: "fail_on_unmatched_files",
            value: Setting::Flag(true),
            since: &[0, 1, 6],
        }),
    },
    // `action_fail_on_inconclusive`, default `false`, in the root action.yml and in each variant's:
    // https://github.com/EnricoMi/publish-unit-test-result-action/tree/da5b7be67623cd59bbbd1a0ce4f022050a55605f (v2.26.0)
    // v2.2.0 lacks it, v2.3.0 declares it:
    // https://github.com/EnricoMi/publish-unit-test-result-action/blob/a3caf02865c0604ad3dc1ecfcc5cdec9c41b7936/action.yml
    FailFast {
        names: &[
            "EnricoMi/publish-unit-test-result-action",
            "EnricoMi/publish-unit-test-result-action/composite",
            "EnricoMi/publish-unit-test-result-action/docker",
            "EnricoMi/publish-unit-test-result-action/linux",
            "EnricoMi/publish-unit-test-result-action/macos",
            "EnricoMi/publish-unit-test-result-action/windows",
            "EnricoMi/publish-unit-test-result-action/windows/bash",
            "EnricoMi/publish-unit-test-result-action/windows/powershell",
        ],
        concepts: &["FailFastPublishUnitTestResults"],
        outcome: "publish-unit-test-result-action passes when it finds no test results",
        looks_with: None,
        input: Some(Input {
            name: "action_fail_on_inconclusive",
            value: Setting::Flag(true),
            since: &[2, 3, 0],
        }),
    },
    // "If there are no changes (i.e. no diff exists with the checked-out base branch), no pull
    // request will be created and the action exits silently", and no input changes that:
    // https://github.com/peter-evans/create-pull-request/blob/3ffccacd46053766916f087ce41d627c0b8cf2d1/README.md (v8.1.3)
    FailFast {
        names: &["peter-evans/create-pull-request"],
        concepts: &["FailFastPeterEvansCreatePullRequest"],
        outcome: "create-pull-request passes without a pull request when there is no diff",
        looks_with: None,
        input: None,
    },
];

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let suppresses = |value: Option<&Value>| {
        matches!(value, Some(Value::Bool(true))) || value.and_then(Value::as_str) == Some("true")
    };
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        if suppresses(job.get("continue-on-error")) {
            let mut finding = common::finding(
                workflow,
                &RULE,
                job.route.with_key("continue-on-error"),
                job.location(),
                "job failures never fail the workflow",
            )?;
            finding.concepts = CONTINUE_ON_ERROR;
            findings.push(finding);
        }
        for step in common::steps(&job) {
            if suppresses(step.value.get("continue-on-error")) {
                let mut finding = common::finding(
                    workflow,
                    &RULE,
                    step.key("continue-on-error"),
                    step.location.clone(),
                    "step failures never fail the job",
                )?;
                finding.concepts = CONTINUE_ON_ERROR;
                findings.push(finding);
            }
            if let Some(finding) = passes_empty(workflow, &step)? {
                findings.push(finding);
            }
        }
    }
    Ok(findings)
}

/// A finding for a step whose action passes when it finds nothing and that does not set the
/// action's fail-fast input. Any value counts as set, as in ghlint: `if-no-files-found: warn` is
/// an explicit opt-out.
fn passes_empty(workflow: &Workflow, step: &common::Step<'_>) -> Result<Option<Finding>, String> {
    let Some(action) = FAIL_FAST
        .iter()
        .find(|action| action.names.iter().any(|name| step.uses_action(name)))
    else {
        return Ok(None);
    };
    let inputs = match step.value.get("with") {
        None | Some(Value::Null) => None,
        Some(Value::Mapping(inputs)) => Some(inputs),
        // Not a literal mapping, so which inputs it sets is unknown.
        Some(_) => return Ok(None),
    };
    // The runner reads input names ignoring case: `ConvertToStepInputs` in
    // https://github.com/actions/runner/blob/8cfa1049f86781d141056082d73802d56cd2b433/src/Sdk/DTPipelines/Pipelines/ObjectTemplating/PipelineTemplateConverter.cs
    let sets = |name: &str| {
        inputs.is_some_and(|inputs| {
            inputs
                .keys()
                .filter_map(Value::as_str)
                .any(|key| key.eq_ignore_ascii_case(name))
        })
    };
    if action.looks_with.is_some_and(|name| !sets(name))
        || action.input.as_ref().is_some_and(|input| sets(input.name))
    {
        return Ok(None);
    }
    let outcome = action.outcome;
    let (message, fix) = match &action.input {
        None => (
            format!(
                "{outcome}; not fixed: no input fails it; open the pull request with `gh pr create`"
            ),
            None,
        ),
        Some(input) if predates(workflow, step, input.since) => (
            format!(
                "{outcome}; not fixed: {} before v{} has no `{}` input; upgrade it",
                action.names[0],
                dotted(input.since),
                input.name
            ),
            None,
        ),
        // yamlpatch adds keys only to an existing mapping, and an empty `with:` holds none.
        Some(input) if step.value.get("with").is_some_and(Value::is_null) => (
            format!(
                "{outcome}; not fixed: the empty `with:` has no mapping to add to; set `{}: {}` under it",
                input.name, input.value
            ),
            None,
        ),
        Some(input) => (
            format!("{outcome}; set `{}: {}`", input.name, input.value),
            Some(common::add_input(step, input.name, input.value.to_value())),
        ),
    };
    let mut finding = common::finding(
        workflow,
        &RULE,
        step.key("uses"),
        step.location.clone(),
        message,
    )?;
    finding.concepts = action.concepts;
    finding.fix = fix;
    Ok(Some(finding))
}

/// Whether the step's ref names a release before `since`. A shorter ref such as `v2` moves with
/// the latest release it names, so only the numbers it has count. A ref without a version (a
/// branch, or a full SHA without a `# vX.Y.Z` comment) counts as current, as in
/// `persist-credentials`.
fn predates(workflow: &Workflow, step: &common::Step<'_>, since: &[u64]) -> bool {
    common::versioned_action(workflow, step).is_some_and(|(_, numbers)| {
        let compared = numbers.len().min(since.len());
        numbers[..compared] < since[..compared]
    })
}

fn dotted(version: &[u64]) -> String {
    let parts: Vec<String> = version.iter().map(u64::to_string).collect();
    parts.join(".")
}
