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
    /// One sentence on what the rule requires; SARIF's short description and help text.
    pub summary: &'static str,
    /// Curated taxonomy tag (`TAXONOMY` in the research workspace's `build_matrix.py`).
    pub tag: &'static str,
    pub severity: Severity,
    /// Lowest persona that runs the rule.
    pub persona: Persona,
    /// `(tool, rule)` pairs for the same concept in zizmor or actionlint. Their IDs are
    /// accepted in ignore comments and config, and the report names them.
    pub equivalents: &'static [(&'static str, &'static str)],
    /// Reads `rules.<id>.config`.
    pub configurable: bool,
}

impl Rule {
    pub fn answers_to(&self, id: &str) -> bool {
        self.id == id || self.equivalents.iter().any(|(_, other)| *other == id)
    }

    /// Whether `id` names this whole rule: its own ID, or the only equivalent from that tool.
    /// zizmor's `unpinned-images` names only the image half of `unpinned-action`.
    pub fn whole(&self, id: &str) -> bool {
        self.id == id
            || self.equivalents.iter().any(|(tool, other)| {
                *other == id
                    && self
                        .equivalents
                        .iter()
                        .filter(|(same, _)| same == tool)
                        .count()
                        == 1
            })
    }

    /// ` (zizmor: a, b; actionlint: c)`, or nothing when no tool has an equivalent.
    pub fn equivalents_suffix(&self) -> String {
        let mut tools: Vec<(&str, Vec<&str>)> = Vec::new();
        for (tool, id) in self.equivalents {
            match tools.iter_mut().find(|(name, _)| name == tool) {
                Some((_, ids)) => ids.push(id),
                None => tools.push((tool, vec![id])),
            }
        }
        if tools.is_empty() {
            return String::new();
        }
        let named: Vec<String> = tools
            .iter()
            .map(|(tool, ids)| format!("{tool}: {}", ids.join(", ")))
            .collect();
        format!(" ({})", named.join("; "))
    }
}

pub struct Finding {
    pub rule: &'static Rule,
    /// The rule's severity unless the config remaps it.
    pub severity: Severity,
    pub message: String,
    pub location: String,
    /// 1-based start of the reported YAML node.
    pub line: usize,
    pub column: usize,
    /// 1-based end, with the column after the node's last character (SARIF `endColumn`).
    pub end_line: usize,
    pub end_column: usize,
    pub route: Route<'static>,
    pub fix: Option<ProposedFix>,
    /// The equivalent ID this finding belongs to when an other tool splits the rule, such as
    /// `unpinned-images` for an image in `unpinned-action`.
    pub concept: Option<&'static str>,
}

impl Finding {
    /// Whether an ignore or config key names this finding: the whole rule, or its concept.
    pub fn answers_to(&self, id: &str) -> bool {
        self.rule.whole(id)
            || (self.rule.answers_to(id) && self.concept.is_none_or(|concept| concept == id))
    }
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

pub use repository::{Located, Parsed, Scanned, local_actions};

/// The rule's `rules.<id>.config` block, if the config has one.
pub type Options<'a> = Option<&'a yaml_serde::Value>;

type Detect = fn(&Workflow) -> Result<Vec<Finding>, String>;
type ConfigurableDetect = fn(&Workflow, Options) -> Result<Vec<Finding>, String>;
type RepositoryDetect = fn(&Path, &Scanned, bool, Options) -> Result<Vec<Located>, String>;
type OnlineDetect = fn(&Workflow, &GitHub, Options) -> Result<Vec<Finding>, String>;

const REGISTRY: &[(&Rule, Detect)] = &[
    // Supply chain
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
    (&maintainability::REMOVED, maintainability::removed),
    // Correctness
    (&correctness::SCHEMA, correctness::schema),
    (&correctness::SCRIPT_LINT, correctness::script_lint),
    (
        &correctness::INSECURE_PROTOCOL,
        correctness::insecure_protocol,
    ),
];

/// Rules that read `rules.<id>.config`, with the option names zizmor documents.
const CONFIGURABLE: &[(&Rule, ConfigurableDetect)] = &[
    (&immutable_references::RULE, immutable_references::detect),
    (&supply_chain::FORBIDDEN, supply_chain::forbidden),
    (&runner::SELF_HOSTED, runner::self_hosted),
];

const ONLINE: &[(&Rule, OnlineDetect)] = &[
    (&online::VULNERABLE, online::vulnerable),
    (&online::OUTDATED, online::outdated),
    (&online::COMMENT_MISMATCH, online::comment_mismatch),
    (&online::IMPOSTOR, online::impostor),
    (&online::ARCHIVED, online::archived),
];

/// Offline stand-ins for online rules, run only without GitHub API access. Their rules are listed
/// through `ONLINE`, so they are not repeated in `all()`.
const OFFLINE: &[(&Rule, Detect)] = &[(&online::ARCHIVED, online::archived_offline)];

const REPOSITORY: &[(&Rule, RepositoryDetect)] = &[
    (&repository::COOLDOWN, repository::cooldown),
    (&repository::TOOLING, repository::tooling),
];

/// Checks that state github.com facts (retired hosted-runner images, service shutdowns, the
/// runner's Node.js) and are skipped for another `--gh-hostname`. `removed-feature` keeps its
/// host-independent part in `REGISTRY`, and `retired-runtime` is listed through
/// `HOSTED_REPOSITORY`, so neither is repeated in `all()`.
const HOSTED: &[(&Rule, Detect)] = &[
    (&maintainability::REMOVED, maintainability::removed_hosted),
    (&repository::RUNTIME, maintainability::retired_runtime),
];
const HOSTED_REPOSITORY: &[(&Rule, RepositoryDetect)] =
    &[(&repository::RUNTIME, repository::runtime)];

/// Every rule with the scope it reads: one workflow, the repository, or the GitHub API.
pub fn all() -> impl Iterator<Item = (&'static Rule, &'static str)> {
    REGISTRY
        .iter()
        .map(|(rule, _)| (*rule, "workflow"))
        .chain(CONFIGURABLE.iter().map(|(rule, _)| (*rule, "workflow")))
        .chain(ONLINE.iter().map(|(rule, _)| (*rule, "online")))
        .chain(REPOSITORY.iter().map(|(rule, _)| (*rule, "repository")))
        .chain(
            HOSTED_REPOSITORY
                .iter()
                .map(|(rule, _)| (*rule, "repository")),
        )
        .chain([(&online::SETTINGS, "repository")])
}

/// Config IDs whose `config` block no crosgha rule reads.
pub fn unused_options(config: &crate::config::Config) -> Vec<String> {
    let readers: Vec<&Rule> = CONFIGURABLE
        .iter()
        .map(|(rule, _)| *rule)
        .chain(ONLINE.iter().map(|(rule, _)| *rule))
        .chain(REPOSITORY.iter().map(|(rule, _)| *rule))
        .chain(HOSTED_REPOSITORY.iter().map(|(rule, _)| *rule))
        .filter(|rule| rule.configurable)
        .collect();
    config
        .configured_ids()
        .filter(|id| !readers.iter().any(|rule| rule.answers_to(id)))
        .map(str::to_owned)
        .collect()
}

/// Actions settings of `repository` (`owner/name`), read through the GitHub API.
pub fn detect_settings(
    repository: &str,
    scanned: &Path,
    settings: &Settings,
) -> Result<Vec<Located>, crate::github::ApiError> {
    match &settings.github {
        Some(github) if enabled(&online::SETTINGS, settings) => Ok(finish_located(
            online::settings(github, repository, scanned)?,
            settings,
        )),
        _ => Ok(Vec::new()),
    }
}

/// Repository-level rules for the `.github` directory next to the scanned workflows.
pub fn detect_repository(
    github: &Path,
    scanned: &Scanned,
    settings: &Settings,
) -> Result<Vec<Located>, String> {
    let mut found = Vec::new();
    let hosted = if settings.hosted {
        HOSTED_REPOSITORY
    } else {
        &[]
    };
    for (rule, detect) in REPOSITORY.iter().chain(hosted) {
        if enabled(rule, settings) {
            found.extend(detect(
                github,
                scanned,
                settings.ignores,
                settings.config.options(rule),
            )?);
        }
    }
    Ok(finish_located(found, settings))
}

/// Runs the Dependabot rule on a config passed as an input path.
pub fn detect_dependabot(path: &Path, settings: &Settings) -> Result<Vec<Located>, String> {
    if !enabled(&repository::COOLDOWN, settings) {
        return Ok(Vec::new());
    }
    let found = repository::cooldown_file(
        path,
        settings.ignores,
        settings.config.options(&repository::COOLDOWN),
    )?;
    Ok(finish_located(found, settings))
}

fn enabled(rule: &'static Rule, settings: &Settings) -> bool {
    rule.persona <= settings.persona && !settings.config.disabled(rule)
}

/// Applies config ignores, severity remaps, and `--min-severity` to findings from `file`.
fn finish(findings: &mut Vec<Finding>, settings: &Settings, file: &str) {
    findings.retain(|finding| !settings.config.drops(finding));
    if settings.ignores {
        findings.retain(|finding| !settings.config.ignores(finding, file));
    }
    for finding in findings.iter_mut() {
        if let Some(severity) = settings.config.severity(finding) {
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
    let hosted = if settings.hosted { HOSTED } else { &[] };
    for (rule, detect) in REGISTRY.iter().chain(hosted) {
        if enabled(rule, settings) {
            findings.extend(detect(workflow)?);
        }
    }
    for (rule, detect) in CONFIGURABLE {
        if enabled(rule, settings) {
            findings.extend(detect(workflow, settings.config.options(rule))?);
        }
    }
    if let Some(github) = &settings.github {
        for (rule, detect) in ONLINE {
            if enabled(rule, settings) {
                findings.extend(detect(workflow, github, settings.config.options(rule))?);
            }
        }
    } else {
        for (rule, detect) in OFFLINE {
            if enabled(rule, settings) {
                findings.extend(detect(workflow)?);
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
        .any(|id| finding.answers_to(id))
}

fn ignore_list(comment: &str) -> Vec<&str> {
    ["crosgha: ignore[", "zizmor: ignore["]
        .iter()
        .filter_map(|marker| comment.split_once(marker))
        .filter_map(|(_, rest)| rest.split_once(']'))
        .flat_map(|(ids, _)| ids.split(',').map(str::trim))
        .collect()
}
