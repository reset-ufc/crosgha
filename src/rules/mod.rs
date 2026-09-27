pub(crate) mod common;
mod correctness;
mod immutable_references;
mod inherited_secrets;
mod injection;
mod job_timeout;
mod maintainability;
mod online;
mod permissions;
mod persist_credentials;
mod repository;
mod runner;
mod supply_chain;
mod token_permissions;

use std::path::Path;

use yamlpatch::Patch;
use yamlpath::Route;

use crate::{
    github::GitHub,
    policy::{Persona, Settings, Severity},
    workflow::Workflow,
};

pub struct Rule {
    pub id: &'static str,
    /// Curated taxonomy tag (`TAXONOMY` in the research workspace's `build_matrix.py`).
    pub tag: &'static str,
    pub severity: Severity,
    /// Lowest persona that runs the rule.
    pub persona: Persona,
    /// IDs other tools use for the same concept; accepted in ignore comments.
    pub aliases: &'static [&'static str],
}

impl Rule {
    pub fn answers_to(&self, id: &str) -> bool {
        self.id == id || self.aliases.contains(&id)
    }
}

pub struct Finding {
    pub rule: &'static Rule,
    /// The rule's severity unless the config remaps it.
    pub severity: Severity,
    pub message: String,
    pub location: String,
    pub line: usize,
    pub route: Route<'static>,
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

pub use repository::Located;

type Detect = fn(&Workflow) -> Result<Vec<Finding>, String>;
type RepositoryDetect = fn(&Path, bool) -> Result<Vec<Located>, String>;
type OnlineDetect = fn(&Workflow, &GitHub) -> Result<Vec<Finding>, String>;

const REGISTRY: &[(&Rule, Detect)] = &[
    // Supply chain
    (&immutable_references::RULE, immutable_references::detect),
    (
        &supply_chain::UNPINNED_PACKAGE,
        supply_chain::unpinned_package,
    ),
    (&supply_chain::REMOTE_SCRIPT, supply_chain::remote_script),
    (&supply_chain::TYPOSQUATTING, supply_chain::typosquatting),
    // Injection and privileged triggers
    (&injection::SCRIPT_INJECTION, injection::script_injection),
    (&injection::ENV_INJECTION, injection::env_injection),
    (&injection::PROMPT_INJECTION, injection::prompt_injection),
    (&injection::PWN_REQUEST, injection::pwn_request),
    (&injection::DANGEROUS_TRIGGER, injection::dangerous_trigger),
    (&injection::UNSAFE_IF, injection::unsafe_if),
    (
        &injection::ARTIFACT_POISONING,
        injection::artifact_poisoning,
    ),
    (&injection::CACHE_POISONING, injection::cache_poisoning),
    (&injection::OBFUSCATION, injection::obfuscation),
    // Permissions and secrets
    (&permissions::EXCESSIVE, permissions::excessive),
    (&token_permissions::RULE, token_permissions::detect),
    (
        &permissions::SECRETS_EXPOSURE,
        permissions::secrets_exposure,
    ),
    (&inherited_secrets::RULE, inherited_secrets::detect),
    (&permissions::HARDCODED, permissions::hardcoded),
    (&persist_credentials::RULE, persist_credentials::detect),
    (&permissions::OIDC, permissions::oidc),
    // Runner
    (&runner::SELF_HOSTED, runner::self_hosted),
    (&runner::MONITORING, runner::monitoring),
    (&runner::VERSIONING, runner::versioning),
    // Maintainability
    (&job_timeout::RULE, job_timeout::detect),
    (
        &maintainability::ERROR_SUPPRESSION,
        maintainability::error_suppression,
    ),
    (&maintainability::CONCURRENCY, maintainability::concurrency),
    (
        &maintainability::BROAD_TRIGGER,
        maintainability::broad_trigger,
    ),
    (&maintainability::FORK_GUARD, maintainability::fork_guard),
    (&maintainability::MATRIX, maintainability::matrix),
    (
        &maintainability::CACHE_MISUSE,
        maintainability::cache_misuse,
    ),
    (
        &maintainability::DUPLICATED_STEPS,
        maintainability::duplicated_steps,
    ),
    (&maintainability::COMPLEXITY, maintainability::complexity),
    (&maintainability::READABILITY, maintainability::readability),
    (&maintainability::DEPRECATED, maintainability::deprecated),
    // Correctness
    (&correctness::SCHEMA, correctness::schema),
    (&correctness::SCRIPT_LINT, correctness::script_lint),
    (
        &correctness::INSECURE_PROTOCOL,
        correctness::insecure_protocol,
    ),
];

const ONLINE: &[(&Rule, OnlineDetect)] = &[
    (&online::VULNERABLE, online::vulnerable),
    (&online::OUTDATED, online::outdated),
    (&online::COMMENT_MISMATCH, online::comment_mismatch),
    (&online::IMPOSTOR, online::impostor),
    (&online::ARCHIVED, online::archived),
];

const REPOSITORY: &[(&Rule, RepositoryDetect)] = &[
    (&repository::COOLDOWN, repository::cooldown),
    (&repository::TOOLING, repository::tooling),
];

/// Every rule with the scope it reads: one workflow, the repository, or the GitHub API.
pub fn all() -> impl Iterator<Item = (&'static Rule, &'static str)> {
    REGISTRY
        .iter()
        .map(|(rule, _)| (*rule, "workflow"))
        .chain(ONLINE.iter().map(|(rule, _)| (*rule, "online")))
        .chain(REPOSITORY.iter().map(|(rule, _)| (*rule, "repository")))
        .chain([(&online::SETTINGS, "repository")])
}

/// Actions settings of `repository` (`owner/name`), read through the GitHub API.
pub fn detect_settings(
    repository: &str,
    scanned: &Path,
    settings: &Settings,
) -> Result<Vec<Located>, String> {
    match &settings.github {
        Some(github) if enabled(&online::SETTINGS, settings) => Ok(finish_located(
            online::settings(github, repository, scanned)?,
            settings,
        )),
        _ => Ok(Vec::new()),
    }
}

/// Repository-level rules for the `.github` directory next to the scanned workflows.
pub fn detect_repository(github: &Path, settings: &Settings) -> Result<Vec<Located>, String> {
    let mut found = Vec::new();
    for (rule, detect) in REPOSITORY {
        if enabled(rule, settings) {
            found.extend(detect(github, settings.ignores)?);
        }
    }
    Ok(finish_located(found, settings))
}

fn enabled(rule: &Rule, settings: &Settings) -> bool {
    rule.persona <= settings.persona && !settings.config.disabled(rule)
}

/// Applies config ignores, severity remaps, and `--min-severity` to findings from `file`.
fn finish(findings: &mut Vec<Finding>, settings: &Settings, file: &str) {
    if settings.ignores {
        findings.retain(|finding| !settings.config.ignores(finding.rule, file, finding.line));
    }
    for finding in findings.iter_mut() {
        if let Some(severity) = settings.config.severity(finding.rule) {
            finding.severity = severity;
        }
    }
    if let Some(minimum) = settings.min_severity {
        findings.retain(|finding| finding.severity >= minimum);
    }
}

/// Runs every enabled rule on the workflow stored in `file` (a base name, for config ignores).
pub fn detect_all(
    workflow: &Workflow,
    settings: &Settings,
    file: &str,
) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for (rule, detect) in REGISTRY {
        if enabled(rule, settings) {
            findings.extend(detect(workflow)?);
        }
    }
    if let Some(github) = &settings.github {
        for (rule, detect) in ONLINE {
            if enabled(rule, settings) {
                findings.extend(detect(workflow, github)?);
            }
        }
    }
    if settings.ignores {
        findings.retain(|finding| !ignored(workflow, finding));
    }
    finish(&mut findings, settings, file);
    findings.sort_by(|left, right| {
        (left.line, left.rule.id, &left.location).cmp(&(right.line, right.rule.id, &right.location))
    });
    Ok(findings)
}

fn finish_located(found: Vec<Located>, settings: &Settings) -> Vec<Located> {
    found
        .into_iter()
        .filter_map(|mut located| {
            let file = located
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut single = vec![located.finding];
            finish(&mut single, settings, &file);
            located.finding = single.pop()?;
            Some(located)
        })
        .collect()
}

/// `# crosgha: ignore[rule,...]` or zizmor's `# zizmor: ignore[rule,...]` anywhere in the
/// finding's YAML span, matching https://docs.zizmor.sh/usage/#with-comments
pub(super) fn ignored(workflow: &Workflow, finding: &Finding) -> bool {
    let Ok(feature) = workflow.document.query_pretty(&finding.route) else {
        return false;
    };
    workflow
        .document
        .feature_comments(&feature)
        .iter()
        .flat_map(|comment| ignore_list(workflow.document.extract(comment)))
        .any(|id| finding.rule.answers_to(id))
}

fn ignore_list(comment: &str) -> Vec<&str> {
    ["crosgha: ignore[", "zizmor: ignore["]
        .iter()
        .filter_map(|marker| comment.split_once(marker))
        .filter_map(|(_, rest)| rest.split_once(']'))
        .flat_map(|(ids, _)| ids.split(',').map(str::trim))
        .collect()
}
