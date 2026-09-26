mod immutable_references;
mod inherited_secrets;
mod job_timeout;
mod persist_credentials;
mod token_permissions;

use yamlpatch::Patch;
use yamlpath::Route;

use crate::{
    policy::{RulePolicy, Severity},
    workflow::Workflow,
};

pub struct Rule {
    pub id: &'static str,
    pub tag: &'static str,
    pub severity: Severity,
}

pub struct Finding {
    pub rule: &'static Rule,
    pub message: &'static str,
    pub location: String,
    pub line: usize,
    pub fix: Option<ProposedFix>,
}

pub struct ProposedFix {
    pub patch: Patch<'static>,
    pub changed_route: Route<'static>,
    pub change: ChangeKind,
}

#[derive(Clone, Copy)]
pub enum ChangeKind {
    Add,
    Replace,
}

pub fn detect_all(workflow: &Workflow, policy: RulePolicy) -> Result<Vec<Finding>, String> {
    let mut findings = persist_credentials::detect(workflow)?;
    if policy.require_immutable_references {
        findings.extend(immutable_references::detect(workflow)?);
    }
    if policy.require_job_timeout {
        findings.extend(job_timeout::detect(workflow)?);
    }
    if policy.require_explicit_token_permissions {
        findings.extend(token_permissions::detect(workflow)?);
    }
    if policy.require_explicit_workflow_secrets {
        findings.extend(inherited_secrets::detect(workflow)?);
    }
    Ok(findings)
}
