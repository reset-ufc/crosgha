use yamlpatch::apply_yaml_patches;

use crate::{
    policy::Settings,
    rules::{self, ChangeKind, Finding, ProposedFix},
    workflow::Workflow,
};

pub struct FixOutcome {
    pub source: String,
    /// One entry per proposed fix, in finding order.
    pub attempts: Vec<Attempt>,
    pub remaining: Vec<Finding>,
}

impl FixOutcome {
    pub fn applied(&self) -> usize {
        self.attempts
            .iter()
            .filter(|attempt| attempt.rejected.is_none())
            .count()
    }
}

pub struct Attempt {
    /// Index of the finding that proposed the fix.
    pub finding: usize,
    /// Why validation refused the fix; `None` when it is part of `FixOutcome::source`.
    pub rejected: Option<String>,
}

/// Applies every proposal that passes validation. A rejected proposal leaves the source as it
/// was, so its finding stays among the remaining ones and the other proposals still run.
pub fn apply(
    workflow: Workflow,
    findings: &[Finding],
    settings: &Settings,
    file: &str,
) -> Result<FixOutcome, String> {
    let mut current = workflow;
    let mut attempts = Vec::new();
    for (index, finding) in findings.iter().enumerate() {
        let Some(proposal) = &finding.fix else {
            continue;
        };
        let rejected = match patch(&current, finding, proposal, settings, file) {
            Ok(next) => {
                current = next;
                None
            }
            Err(reason) => Some(reason),
        };
        attempts.push(Attempt {
            finding: index,
            rejected,
        });
    }
    let remaining = rules::detect_all(&current, settings, file)?;
    Ok(FixOutcome {
        source: current.document.source().to_owned(),
        attempts,
        remaining,
    })
}

fn patch(
    current: &Workflow,
    finding: &Finding,
    proposal: &ProposedFix,
    settings: &Settings,
    file: &str,
) -> Result<Workflow, String> {
    let before = rules::detect_all(current, settings, file)?;
    let patched = apply_yaml_patches(&current.document, std::slice::from_ref(&proposal.patch))
        .map_err(|error| error.to_string())?;
    let next = Workflow::parse(patched.source().to_owned())?;
    validate_change(current, &next, finding, proposal, &before, settings, file)?;
    Ok(next)
}

fn validate_change(
    before: &Workflow,
    after: &Workflow,
    target: &Finding,
    proposal: &ProposedFix,
    old_findings: &[Finding],
    settings: &Settings,
    file: &str,
) -> Result<(), String> {
    let new_findings = rules::detect_all(after, settings, file)?;
    if new_findings
        .iter()
        .any(|finding| same_target(finding, target))
        || new_findings.len() + 1 != old_findings.len()
    {
        return Err("fix did not remove exactly the targeted finding".to_owned());
    }
    if new_findings
        .iter()
        .any(|finding| !old_findings.iter().any(|old| same_target(old, finding)))
    {
        return Err("fix introduced a new finding".to_owned());
    }

    let route = &proposal.changed_route;
    let only_target = match proposal.change {
        ChangeKind::Add => {
            let span = after
                .document
                .removal_span(route)
                .map_err(|error| error.to_string())?;
            let restored = format!(
                "{}{}",
                &after.document.source()[..span.start],
                &after.document.source()[span.end..]
            );
            restored == before.document.source()
        }
        ChangeKind::Replace => {
            let old = before
                .document
                .query_exact(route)
                .map_err(|error| error.to_string())?
                .ok_or("missing original value")?;
            let new = after
                .document
                .query_exact(route)
                .map_err(|error| error.to_string())?
                .ok_or("missing replacement value")?;
            let (start, end) = new.location.byte_span;
            let restored = format!(
                "{}{}{}",
                &after.document.source()[..start],
                before.document.extract(&old),
                &after.document.source()[end..]
            );
            restored == before.document.source()
        }
    };
    if !only_target {
        return Err("fix changed YAML outside the intended node".to_owned());
    }
    Ok(())
}

fn same_target(left: &Finding, right: &Finding) -> bool {
    left.rule.id == right.rule.id && left.location == right.location
}
