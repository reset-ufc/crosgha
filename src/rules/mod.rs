mod persist_credentials;

use yamlpatch::Patch;
use yamlpath::Route;

use crate::{policy::Severity, workflow::Workflow};

pub struct Rule {
    pub id: &'static str,
    pub tag: &'static str,
    pub severity: Severity,
}

pub struct Finding {
    pub rule: &'static Rule,
    pub message: &'static str,
    pub job: String,
    pub step: usize,
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

pub fn detect_all(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    persist_credentials::detect(workflow)
}
