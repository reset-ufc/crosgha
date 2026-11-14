use yaml_serde::Value;
use yamlpath::Route;

use super::{ChangeKind, Edit, Finding, ProposedFix, Rule, common, expression, shell};
use crate::policy::{Persona, Severity};
use crate::workflow::Workflow;

// zizmor `anonymous-definition` and Khatami et al. (SCAM 2024), KZ-16 "Name run steps". The ghlint
// concepts follow its rules at 74115a437cfa161c2fd2d72fc2ebcd83ac514b80
// (https://github.com/TWiStErRob/net.twisterrob.ghlint/tree/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules):
// names from `MissingNameRule.kt`, IDs from `IdNamingRule.kt` and `DuplicateStepIdRule.kt`, shells
// from `MissingShellRule.kt`, the token from `PreferGitHubTokenRule.kt`, and conditions from the
// bare branch of `DoubleCurlyIf` (`DoubleCurlyIfRule.kt`) and from `NeverUseAlways` and
// `NegativeStatusCheck` (`ImplicitStatusCheckRule.kt`). GitHub accepts every shape, so the rule
// stays at `auditor`.
pub const RULE: Rule = Rule {
    id: "readability",
    summary: "Workflows, jobs, and run steps should have names, lower-case kebab IDs, and a stated shell; conditions and token references should say what they mean.",
    tag: "readability",
    severity: Severity::Note,
    persona: Persona::Auditor,
    equivalents: &[
        ("zizmor", "anonymous-definition"),
        ("ghlint", "MissingWorkflowName"),
        ("ghlint", "MissingJobName"),
        ("ghlint", "MissingStepName"),
        ("ghlint", "WorkflowIdNaming"),
        ("ghlint", "JobIdNaming"),
        ("ghlint", "StepIdNaming"),
        ("ghlint", "SimilarStepId"),
        ("ghlint", "MissingShell"),
        ("ghlint", "PreferGitHubToken"),
        ("ghlint", "DoubleCurlyIf"),
        ("ghlint", "NeverUseAlways"),
        ("ghlint", "NegativeStatusCheck"),
    ],
    configurable: false,
    relational: false,
};

/// ghlint's `LOWER_KEBAB` (`^[a-z0-9-]+$`) for workflow, job, and step IDs.
fn kebab(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// ghlint's `MAX_EDIT_DISTANCE`: two step IDs this close are easy to confuse, and identical ones
/// (distance 0) are a schema error instead.
const MAX_EDIT_DISTANCE: usize = 2;

/// Edits that turn `left` into `right`: insertions, deletions, substitutions, and swaps of two
/// adjacent characters, each character edited once (ghlint's `editDistance.kt`).
fn edit_distance(left: &str, right: &str) -> usize {
    let (left, right): (Vec<char>, Vec<char>) = (left.chars().collect(), right.chars().collect());
    let mut table = vec![vec![0; right.len() + 1]; left.len() + 1];
    for (index, row) in table.iter_mut().enumerate() {
        row[0] = index;
    }
    for (index, cell) in table[0].iter_mut().enumerate() {
        *cell = index;
    }
    for i in 1..=left.len() {
        for j in 1..=right.len() {
            let substitution = usize::from(left[i - 1] != right[j - 1]);
            let mut best = (table[i - 1][j] + 1)
                .min(table[i][j - 1] + 1)
                .min(table[i - 1][j - 1] + substitution);
            if i > 1 && j > 1 && left[i - 1] == right[j - 2] && left[i - 2] == right[j - 1] {
                best = best.min(table[i - 2][j - 2] + 1);
            }
            table[i][j] = best;
        }
    }
    table[left.len()][right.len()]
}

/// The automatic token's name in the `secrets` context, lowercase.
const TOKEN_SECRET: &str = "github_token";

/// A step's string values, top-level or one mapping down (`with.<input>`, `env.<name>`), with their
/// route and key path. The docs set `github.token` only within a job's steps and leave it `null`
/// elsewhere, such as in a reusable workflow call's `with` and `secrets`
/// (https://docs.github.com/en/actions/reference/workflows-and-actions/contexts), so only step
/// values are read. `if` cannot read `secrets` at all.
fn step_strings<'a>(step: &common::Step<'a>) -> Vec<(Route<'static>, String, &'a str)> {
    let mut found = Vec::new();
    let Some(mapping) = step.value.as_mapping() else {
        return found;
    };
    for (key, value) in mapping {
        let Some(key) = key.as_str().filter(|key| *key != "if") else {
            continue;
        };
        match value {
            Value::String(text) => found.push((step.key(key), key.to_owned(), text.as_str())),
            Value::Mapping(entries) => {
                for (name, value) in entries {
                    if let (Some(name), Some(text)) = (name.as_str(), value.as_str()) {
                        found.push((
                            step.key(key).with_key(name.to_owned()),
                            format!("{key}.{name}"),
                            text,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// Rewrites every `secrets.GITHUB_TOKEN` in the scalar at `route` to `github.token`, in its source,
/// so its style and comments stay. The source must hold the references `value` holds.
fn token_fix(workflow: &Workflow, route: &Route<'static>, value: &str) -> Option<ProposedFix> {
    let source = common::own_scalar(workflow, route)?;
    let references = expression::secret_references(source, TOKEN_SECRET);
    if references.len() != expression::secret_references(value, TOKEN_SECRET).len() {
        return None;
    }
    let mut text = String::with_capacity(source.len());
    let mut from = 0;
    for reference in references {
        text.push_str(&source[from..reference.start]);
        text.push_str("github.token");
        from = reference.end;
    }
    text.push_str(&source[from..]);
    Some(ProposedFix {
        edit: Edit::Scalar(text),
        changed_route: route.clone(),
        change: ChangeKind::Replace,
    })
}

fn report(
    workflow: &Workflow,
    route: Route<'static>,
    location: impl Into<String>,
    message: impl Into<String>,
    concepts: &'static [&'static str],
) -> Result<Finding, String> {
    let mut finding = common::finding(workflow, &RULE, route, location, message)?;
    finding.concepts = concepts;
    Ok(finding)
}

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    if workflow.value.get("name").is_none() {
        findings.push(report(
            workflow,
            Route::default().with_key("on"),
            "workflow",
            "workflow has no `name`",
            &["MissingWorkflowName"],
        )?);
    }
    let workflow_shell = shell::default_shell(&workflow.value);
    for job in common::jobs(workflow) {
        if !kebab(job.name) {
            findings.push(report(
                workflow,
                job.route.clone(),
                job.location(),
                "job ID is not lower-case kebab (`a-z`, `0-9`, `-`)",
                &["JobIdNaming"],
            )?);
        }
        // ghlint counts a blank name as missing.
        let named = job
            .get("name")
            .is_some_and(|name| name.as_str().is_none_or(|name| !name.trim().is_empty()));
        if !named {
            findings.push(report(
                workflow,
                job.route.clone(),
                job.location(),
                "job has no `name`",
                &["MissingJobName"],
            )?);
        }
        let default_shell = shell::default_shell(job.value).or(workflow_shell);
        let mut ids: Vec<&str> = Vec::new();
        for step in common::steps(&job) {
            findings.extend(step_findings(workflow, &step, default_shell)?);
            // Each pair is reported once, at the later step; a repeated ID adds no pair.
            let Some(id) = step
                .value
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !ids.contains(id))
            else {
                continue;
            };
            let similar: Vec<String> = ids
                .iter()
                .filter(|earlier| (1..=MAX_EDIT_DISTANCE).contains(&edit_distance(earlier, id)))
                .map(|earlier| format!("`{earlier}`"))
                .collect();
            if !similar.is_empty() {
                findings.push(report(
                    workflow,
                    step.key("id"),
                    step.location.clone(),
                    format!(
                        "step ID `{id}` is easy to confuse with {}",
                        similar.join(", ")
                    ),
                    &["SimilarStepId"],
                )?);
            }
            ids.push(id);
        }
    }
    for condition in common::conditions(workflow) {
        for (message, concepts) in unclear_condition(condition.value) {
            findings.push(report(
                workflow,
                condition.route.clone(),
                condition.location.clone(),
                message,
                concepts,
            )?);
        }
    }
    Ok(findings)
}

/// Names, IDs, the shell, and token references of one step. `default_shell` is the job's or the
/// workflow's `defaults.run.shell`.
fn step_findings(
    workflow: &Workflow,
    step: &common::Step<'_>,
    default_shell: Option<&str>,
) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    if step.run().is_some() && step.value.get("name").is_none() {
        findings.push(report(
            workflow,
            step.key("run"),
            step.location.clone(),
            "`run` step has no `name`",
            &["MissingStepName"],
        )?);
    }
    if step.run().is_some() && step.value.get("shell").is_none() && default_shell.is_none() {
        findings.push(report(
            workflow,
            step.key("run"),
            step.location.clone(),
            "`run` step has no `shell`, in the step or in `defaults.run`",
            &["MissingShell"],
        )?);
    }
    if let Some(id) = step.value.get("id").and_then(Value::as_str)
        && !kebab(id)
    {
        findings.push(report(
            workflow,
            step.key("id"),
            step.location.clone(),
            "step ID is not lower-case kebab (`a-z`, `0-9`, `-`)",
            &["StepIdNaming"],
        )?);
    }
    // A `parallel` group's values belong to its steps, which are visited on their own.
    if step.value.get("parallel").is_some() {
        return Ok(findings);
    }
    for (route, key, text) in step_strings(step) {
        if expression::secret_references(text, TOKEN_SECRET).is_empty() {
            continue;
        }
        let mut finding = report(
            workflow,
            route.clone(),
            format!("{}, `{key}`", step.location),
            "`secrets.GITHUB_TOKEN` reads like a repository secret; the automatic token is `github.token`",
            &["PreferGitHubToken"],
        )?;
        finding.fix = token_fix(workflow, &route, text);
        findings.push(finding);
    }
    Ok(findings)
}

/// Flags the workflow's file name (`file`, without `.yml` or `.yaml`, is its ID) when it is not
/// lower-case kebab.
pub fn file_name(workflow: &Workflow, file: &str) -> Result<Vec<Finding>, String> {
    let id = file
        .strip_suffix(".yml")
        .or_else(|| file.strip_suffix(".yaml"))
        .unwrap_or(file);
    if id.is_empty() || kebab(id) {
        return Ok(Vec::new());
    }
    Ok(vec![report(
        workflow,
        Route::default().with_key("on"),
        "workflow",
        format!("file name `{file}` is not lower-case kebab (`a-z`, `0-9`, `-`)"),
        &["WorkflowIdNaming"],
    )?])
}

/// What hides the intent of an `if`, with the ghlint concept of each.
fn unclear_condition(condition: &Value) -> Vec<(String, &'static [&'static str])> {
    let mut problems: Vec<(String, &'static [&'static str])> = Vec::new();
    let text = match condition {
        Value::String(text) => text,
        // `if: !cancelled()` and `if: ! cancelled()` start a YAML tag, which takes the `!`.
        Value::Tagged(_) => {
            problems.push((
                "YAML reads the leading `!` as a tag; wrap the condition in `${{ }}`".to_owned(),
                &["DoubleCurlyIf"],
            ));
            return problems;
        }
        _ => return problems,
    };
    // A bare condition becomes a string as soon as someone adds `${{ }}` around one part.
    if !text.contains("${{") && !text.trim().is_empty() {
        problems.push((
            "condition is not wrapped in `${{ }}`".to_owned(),
            &["DoubleCurlyIf"],
        ));
    }
    let checks = expression::status_checks(text);
    if checks.iter().any(|check| check.name == "always") {
        problems.push((
            "`always()` also runs after the run is cancelled; `success() || failure()` does not"
                .to_owned(),
            &["NeverUseAlways"],
        ));
    }
    if let Some(check) = checks.iter().find(|check| check.negated) {
        problems.push((
            format!(
                "`!{}()` names the statuses that skip the work; name the ones that run it, such as `success() || failure()`",
                check.name
            ),
            &["NegativeStatusCheck"],
        ));
    }
    problems
}
