use yaml_serde::Value;
use yamlpath::Route;

use super::{Finding, Rule, common, shell};
use crate::policy::{Persona, Severity};
use crate::workflow::{Kind, Workflow};

// ghlint at 74115a437cfa161c2fd2d72fc2ebcd83ac514b80: `EmptyEnvRule.kt`, `RedundantShellRule.kt`,
// and `DuplicateShellRule.kt` in
// https://github.com/TWiStErRob/net.twisterrob.ghlint/tree/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules
// GitHub runs each of these shapes as intended, so the rule stays at `auditor`.
pub const RULE: Rule = Rule {
    id: "redundant-setting",
    summary: "`env` should not be empty, a `shell` should not repeat the default it inherits, and steps that share a shell should set it once in `defaults.run.shell`.",
    tag: "redundant/duplicated-steps",
    severity: Severity::Note,
    persona: Persona::Auditor,
    equivalents: &[
        ("ghlint", "EmptyWorkflowEnv"),
        ("ghlint", "EmptyJobEnv"),
        ("ghlint", "EmptyStepEnv"),
        ("ghlint", "RedundantShell"),
        ("ghlint", "RedundantDefaultShell"),
        ("ghlint", "DuplicateShellOnSteps"),
    ],
    configurable: false,
    relational: false,
};

/// ghlint's `MAX_SHELLS_ON_STEPS`: this many steps setting one shell, with no default to change,
/// should set it in the job's `defaults.run.shell` instead.
const MIN_REPEATED_SHELLS: usize = 2;

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

/// An `env` written as `{}`. GitHub merges the workflow, job, and step `env` maps, so an empty one
/// adds nothing. ghlint reads only a mapping as empty: `env:` with no value, or only comments under
/// it, is null, and a string is an expression that yields the map at run time.
fn empty_env(
    workflow: &Workflow,
    scope: &Value,
    route: &Route<'static>,
    location: String,
    concepts: &'static [&'static str],
) -> Result<Option<Finding>, String> {
    if !scope
        .get("env")
        .and_then(Value::as_mapping)
        .is_some_and(|env| env.is_empty())
    {
        return Ok(None);
    }
    let route = route.with_key("env");
    let mut finding = report(
        workflow,
        route.clone(),
        location,
        "empty `env` sets nothing",
        concepts,
    )?;
    finding.fix = Some(common::remove_key(route));
    Ok(Some(finding))
}

fn shell_route(scope: &Route<'static>) -> Route<'static> {
    scope.with_key("defaults").with_key("run").with_key("shell")
}

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    let root = Route::default();
    findings.extend(empty_env(
        workflow,
        &workflow.value,
        &root,
        "workflow".to_owned(),
        &["EmptyWorkflowEnv"],
    )?);
    let workflow_shell = shell::default_shell(&workflow.value);
    // The shell of every `run` step in the workflow, `None` where a step sets none.
    let mut all_shells: Vec<Option<&str>> = Vec::new();
    for job in common::jobs(workflow) {
        findings.extend(empty_env(
            workflow,
            job.value,
            &job.route,
            job.location(),
            &["EmptyJobEnv"],
        )?);
        let job_shell = shell::default_shell(job.value);
        if workflow_shell.is_some() && job_shell == workflow_shell {
            let route = shell_route(&job.route);
            let mut finding = report(
                workflow,
                route.clone(),
                job.location(),
                "`defaults.run.shell` repeats the workflow's",
                &["RedundantDefaultShell"],
            )?;
            // The job keeps the shell it inherits from the workflow.
            finding.fix = Some(common::remove_key(route));
            findings.push(finding);
        }
        // ghlint compares a step's shell only with the nearest `defaults.run.shell`, never with the
        // runner's default: an explicit `bash` adds `--noprofile --norc -o pipefail` to the `bash -e
        // {0}` that runs when no shell is set, and Windows runners default to pwsh:
        // https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idstepsshell
        let (default, owner) = match job_shell {
            Some(shell) => (Some(shell), "job's"),
            None => (workflow_shell, "workflow's"),
        };
        let mut shells = Vec::new();
        for step in common::steps(&job) {
            findings.extend(empty_env(
                workflow,
                step.value,
                &step.route,
                step.location.clone(),
                &["EmptyStepEnv"],
            )?);
            if step.run().is_none() {
                continue;
            }
            let own = step.value.get("shell").and_then(Value::as_str);
            shells.push(own);
            if own.is_some() && own == default {
                let route = step.key("shell");
                let mut finding = report(
                    workflow,
                    route.clone(),
                    step.location.clone(),
                    format!("`shell` repeats the {owner} `defaults.run.shell`"),
                    &["RedundantShell"],
                )?;
                finding.fix = Some(common::remove_key(route));
                findings.push(finding);
            }
        }
        if let Some(shell) = common_shell(&shells) {
            let count = shells.len();
            // A composite action has no `defaults`, so each `run` step must set its shell.
            if job.kind == Kind::Workflow
                && job_shell.is_none()
                && workflow_shell.is_none()
                && count >= MIN_REPEATED_SHELLS
            {
                findings.push(report(
                    workflow,
                    job.route.clone(),
                    job.location(),
                    format!(
                        "all {count} `run` steps set `shell: {shell}`; set it once in `defaults.run.shell`"
                    ),
                    &["DuplicateShellOnSteps"],
                )?);
            }
            if let Some(default) = job_shell.filter(|default| *default != shell) {
                findings.push(report(
                    workflow,
                    shell_route(&job.route),
                    job.location(),
                    format!(
                        "all {count} `run` steps override `{default}` with `shell: {shell}`; change the default instead"
                    ),
                    &["DuplicateShellOnSteps"],
                )?);
            }
        }
        all_shells.extend(shells);
    }
    if let (Some(shell), Some(default)) = (common_shell(&all_shells), workflow_shell)
        && shell != default
    {
        findings.push(report(
            workflow,
            shell_route(&root),
            "workflow",
            format!(
                "every `run` step overrides `{default}` with `shell: {shell}`; change the default instead"
            ),
            &["DuplicateShellOnSteps"],
        )?);
    }
    Ok(findings)
}

/// The shell that every one of `shells` sets, if they all set the same one.
fn common_shell<'a>(shells: &[Option<&'a str>]) -> Option<&'a str> {
    let first = (*shells.first()?)?;
    shells
        .iter()
        .all(|shell| *shell == Some(first))
        .then_some(first)
}
