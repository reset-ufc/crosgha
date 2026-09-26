use yamlpatch::apply_yaml_patches;

use crate::{
    policy::RulePolicy,
    rules::{self, ChangeKind, Finding, ProposedFix},
    workflow::Workflow,
};

pub struct FixOutcome {
    pub source: String,
    pub fixed_count: usize,
    pub remaining: Vec<Finding>,
}

pub fn apply(
    mut current: Workflow,
    findings: &[Finding],
    policy: RulePolicy,
) -> Result<FixOutcome, String> {
    let mut fixed_count = 0;
    for finding in findings {
        let Some(proposal) = &finding.fix else {
            continue;
        };
        let before = rules::detect_all(&current, policy)?;
        let patched = apply_yaml_patches(&current.document, std::slice::from_ref(&proposal.patch))
            .map_err(|error| error.to_string())?;
        let next = Workflow::parse(patched.source().to_owned())?;
        validate_change(&current, &next, finding, proposal, &before, policy)?;
        current = next;
        fixed_count += 1;
    }
    let remaining = rules::detect_all(&current, policy)?;
    Ok(FixOutcome {
        source: current.document.source().to_owned(),
        fixed_count,
        remaining,
    })
}

fn validate_change(
    before: &Workflow,
    after: &Workflow,
    target: &Finding,
    proposal: &ProposedFix,
    old_findings: &[Finding],
    policy: RulePolicy,
) -> Result<(), String> {
    let new_findings = rules::detect_all(after, policy)?;
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
    let restored = match proposal.change {
        ChangeKind::Add => {
            let span = after
                .document
                .removal_span(route)
                .map_err(|error| error.to_string())?;
            format!(
                "{}{}",
                &after.document.source()[..span.start],
                &after.document.source()[span.end..]
            )
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
            format!(
                "{}{}{}",
                &after.document.source()[..start],
                before.document.extract(&old),
                &after.document.source()[end..]
            )
        }
    };
    if restored != before.document.source() {
        return Err("fix changed YAML outside the intended node".to_owned());
    }
    Ok(())
}

fn same_target(left: &Finding, right: &Finding) -> bool {
    left.rule.id == right.rule.id && left.location == right.location
}
