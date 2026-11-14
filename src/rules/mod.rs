pub(crate) mod common;
mod correctness;
mod deprecation;
mod env_injection;
mod error_suppression;
pub(crate) mod expression;
mod gh_cli_environment;
mod immutable_references;
mod inherited_secrets;
mod injection;
mod job_timeout;
mod maintainability;
mod online;
mod permissions;
mod persist_credentials;
mod readability;
mod redundant_setting;
mod repository;
mod runner;
mod shell;
mod supply_chain;
mod token_permissions;

use std::path::Path;

use yamlpatch::Patch;
use yamlpath::Route;

use crate::github::GitHub;
use crate::policy::{Persona, Settings, Severity};
use crate::workflow::{Kind, Workflow};

pub struct Rule {
    pub id: &'static str,
    /// One sentence on what the rule requires; SARIF's short description and help text.
    pub summary: &'static str,
    /// Curated taxonomy tag (`TAXONOMY` in the research workspace's `build_matrix.py`).
    pub tag: &'static str,
    pub severity: Severity,
    /// Lowest persona that runs the rule.
    pub persona: Persona,
    /// `(tool, rule)` pairs for the same concept in zizmor, actionlint, or ghlint. Their IDs are
    /// accepted in ignore comments and config, and the report names them.
    pub equivalents: &'static [(&'static str, &'static str)],
    /// Reads `rules.<id>.config`.
    pub configurable: bool,
    /// Compares steps with each other: fixing one of two equal steps clears their finding until
    /// the other step gets the same fix, which restores it.
    pub relational: bool,
}

/// Equivalent IDs that cover only one branch of their rule even when their tool lists no second ID.
const PARTIAL_EQUIVALENTS: &[&str] = &["DoubleCurlyIf"];

impl Rule {
    pub fn answers_to(&self, id: &str) -> bool {
        self.id == id || self.equivalents.iter().any(|(_, other)| *other == id)
    }

    /// Whether `id` names this whole rule: its own ID, or the only equivalent from that tool when
    /// that ID is not partial. zizmor's `unpinned-images` names only the image half of
    /// `unpinned-action`, and ghlint's `DoubleCurlyIf` only the mixed-form branch of
    /// `unsafe-if-condition`.
    pub fn whole(&self, id: &str) -> bool {
        self.id == id
            || (!PARTIAL_EQUIVALENTS.contains(&id)
                && self.equivalents.iter().any(|(tool, other)| {
                    *other == id
                        && self
                            .equivalents
                            .iter()
                            .filter(|(same, _)| same == tool)
                            .count()
                            == 1
                }))
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
    /// The equivalent IDs this finding belongs to when another tool splits the rule, such as
    /// `unpinned-images` for an image in `unpinned-action`. A part no tool names gets a label of its
    /// own (`continue-on-error`), since an empty list answers to every partial ID.
    pub concepts: &'static [&'static str],
}

impl Finding {
    /// Whether an ignore or config key names this finding: the whole rule, or one of its concepts.
    pub fn answers_to(&self, id: &str) -> bool {
        self.rule.whole(id)
            || (self.rule.answers_to(id)
                && (self.concepts.is_empty() || self.concepts.contains(&id)))
    }
}

pub struct ProposedFix {
    pub edit: Edit,
    pub changed_route: Route<'static>,
    pub change: ChangeKind,
}

pub enum Edit {
    Patch(Patch<'static>),
    /// Whole lines of the scalar at `changed_route`, rewritten in order. yamlpatch's `Replace`
    /// re-indents a block scalar and drops its header comment, and its `RewriteFragment` needs a
    /// crate outside the allowed dependencies.
    Lines(Vec<LineRewrite>),
    /// The new source of the scalar at `changed_route`, from its block header or quote to its last
    /// character. The caller keeps the header's comment and the scalar's style.
    Scalar(String),
}

/// Replaces the next line whose text, without its indentation and trailing spaces, is `from`.
pub struct LineRewrite {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Copy)]
pub enum ChangeKind {
    Add,
    Replace,
    Remove,
}

pub use common::local_actions;
pub use repository::{Located, Place, Scanned};

/// The rule's `rules.<id>.config` block, if the config has one.
pub type Options<'a> = Option<&'a yaml_serde::Value>;

type Detect = fn(&Workflow) -> Result<Vec<Finding>, String>;
type NamedDetect = fn(&Workflow, &str) -> Result<Vec<Finding>, String>;
type ConfigurableDetect = fn(&Workflow, Options) -> Result<Vec<Finding>, String>;
type RepositoryDetect = fn(&Path, &Scanned, bool, Options) -> Result<Vec<Located>, String>;
type OnlineDetect = fn(&Workflow, &GitHub, Options) -> Result<Vec<Finding>, String>;

/// What a check reads, which decides the files it runs on. A composite action's `runs.steps` run
/// inside the caller's job, so it has no triggers, jobs, `permissions`, timeouts, or concurrency:
/// https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax#runs-for-composite-actions
#[derive(Clone, Copy, Eq, PartialEq)]
enum Reads {
    /// Keys only a workflow has: triggers, jobs, `permissions`, `defaults`, the job's runner.
    Workflow,
    /// Steps, `uses` references, or text anywhere in the file, so it also runs on action metadata
    /// files, whose composite `runs` is one job (`common::jobs`).
    Steps,
    /// The metadata of an action file: `runs.using`, `runs.image`, and the required keys.
    Metadata,
}

impl Reads {
    fn covers(self, kind: Kind) -> bool {
        match self {
            Reads::Workflow => kind == Kind::Workflow,
            Reads::Steps => true,
            Reads::Metadata => kind == Kind::Action,
        }
    }
}

const REGISTRY: &[(&Rule, Detect, Reads)] = &[
    // Supply chain
    (
        &supply_chain::UNPINNED_PACKAGE,
        supply_chain::unpinned_package,
        Reads::Steps,
    ),
    (
        &supply_chain::REMOTE_SCRIPT,
        supply_chain::remote_script,
        Reads::Steps,
    ),
    (
        &supply_chain::TYPOSQUATTING,
        supply_chain::typosquatting,
        Reads::Steps,
    ),
    // Injection and privileged triggers
    (
        &injection::SCRIPT_INJECTION,
        injection::script_injection,
        Reads::Steps,
    ),
    (&env_injection::RULE, env_injection::detect, Reads::Steps),
    (
        &injection::PROMPT_INJECTION,
        injection::prompt_injection,
        Reads::Steps,
    ),
    (
        &injection::PWN_REQUEST,
        injection::pwn_request,
        Reads::Workflow,
    ),
    (
        &injection::DANGEROUS_TRIGGER,
        injection::dangerous_trigger,
        Reads::Workflow,
    ),
    (&injection::UNSAFE_IF, injection::unsafe_if, Reads::Steps),
    (
        &injection::ARTIFACT_POISONING,
        injection::artifact_poisoning,
        Reads::Workflow,
    ),
    (
        &injection::CACHE_POISONING,
        injection::cache_poisoning,
        Reads::Workflow,
    ),
    (
        &injection::OBFUSCATION,
        injection::obfuscation,
        Reads::Steps,
    ),
    // Permissions and secrets
    (
        &permissions::EXCESSIVE,
        permissions::excessive,
        Reads::Workflow,
    ),
    (
        &permissions::WORKFLOW_LEVEL,
        permissions::workflow_level,
        Reads::Workflow,
    ),
    (
        &token_permissions::RULE,
        token_permissions::detect,
        Reads::Workflow,
    ),
    // Composite steps cannot read `secrets` at all (no step key of `action-v1.0.json` lists the
    // context, actions/languageservices at 4043eda158e16579cc5fb1b0b07a4bce2a76f0b5).
    (
        &permissions::SECRETS_EXPOSURE,
        permissions::secrets_exposure,
        Reads::Workflow,
    ),
    (
        &inherited_secrets::RULE,
        inherited_secrets::detect,
        Reads::Workflow,
    ),
    (
        &permissions::HARDCODED,
        permissions::hardcoded,
        Reads::Steps,
    ),
    (
        &persist_credentials::RULE,
        persist_credentials::detect,
        Reads::Steps,
    ),
    (&permissions::OIDC, permissions::oidc, Reads::Workflow),
    // Runner
    (&runner::MONITORING, runner::monitoring, Reads::Workflow),
    (&runner::VERSIONING, runner::versioning, Reads::Workflow),
    // Maintainability
    (&job_timeout::RULE, job_timeout::detect, Reads::Workflow),
    (
        &error_suppression::RULE,
        error_suppression::detect,
        Reads::Steps,
    ),
    (
        &maintainability::CONCURRENCY,
        maintainability::concurrency,
        Reads::Workflow,
    ),
    (
        &maintainability::BROAD_TRIGGER,
        maintainability::broad_trigger,
        Reads::Workflow,
    ),
    (
        &maintainability::FORK_GUARD,
        maintainability::fork_guard,
        Reads::Workflow,
    ),
    (
        &maintainability::MATRIX,
        maintainability::matrix,
        Reads::Workflow,
    ),
    (
        &maintainability::CACHE_MISUSE,
        maintainability::cache_misuse,
        Reads::Steps,
    ),
    (
        &maintainability::DUPLICATED_STEPS,
        maintainability::duplicated_steps,
        Reads::Steps,
    ),
    (
        &redundant_setting::RULE,
        redundant_setting::detect,
        Reads::Steps,
    ),
    (
        &maintainability::COMPLEXITY,
        maintainability::complexity,
        Reads::Steps,
    ),
    (&readability::RULE, readability::detect, Reads::Steps),
    (
        &deprecation::DEPRECATED,
        deprecation::deprecated,
        Reads::Steps,
    ),
    (&deprecation::REMOVED, deprecation::removed, Reads::Steps),
    // Correctness
    (&correctness::SCHEMA, correctness::schema, Reads::Workflow),
    (
        &correctness::SCHEMA,
        correctness::action_schema,
        Reads::Metadata,
    ),
    (
        &correctness::SCRIPT_LINT,
        correctness::script_lint,
        Reads::Steps,
    ),
    (
        &correctness::INSECURE_PROTOCOL,
        correctness::insecure_protocol,
        Reads::Steps,
    ),
    // A composite action runs in the caller's job, whose `env` and checkout it cannot see.
    (
        &gh_cli_environment::RULE,
        gh_cli_environment::detect,
        Reads::Workflow,
    ),
];

/// Checks that read the workflow's file name. An action's file name is fixed.
const NAMED: &[(&Rule, NamedDetect, Reads)] =
    &[(&readability::RULE, readability::file_name, Reads::Workflow)];

/// Rules that read `rules.<id>.config`, with the option names zizmor documents.
const CONFIGURABLE: &[(&Rule, ConfigurableDetect, Reads)] = &[
    (
        &immutable_references::RULE,
        immutable_references::detect,
        Reads::Steps,
    ),
    (
        &immutable_references::RULE,
        immutable_references::action_image,
        Reads::Metadata,
    ),
    (
        &supply_chain::FORBIDDEN,
        supply_chain::forbidden,
        Reads::Steps,
    ),
    (&runner::SELF_HOSTED, runner::self_hosted, Reads::Workflow),
];

const ONLINE: &[(&Rule, OnlineDetect, Reads)] = &[
    (&online::VULNERABLE, online::vulnerable, Reads::Steps),
    (&online::OUTDATED, online::outdated, Reads::Steps),
    (
        &online::COMMENT_MISMATCH,
        online::comment_mismatch,
        Reads::Steps,
    ),
    (&online::IMPOSTOR, online::impostor, Reads::Steps),
    (&online::ARCHIVED, online::archived, Reads::Steps),
];

/// Offline stand-ins for online rules, run only without GitHub API access.
const OFFLINE: &[(&Rule, Detect, Reads)] =
    &[(&online::ARCHIVED, online::archived_offline, Reads::Steps)];

const REPOSITORY: &[(&Rule, RepositoryDetect)] = &[
    (&repository::COOLDOWN, repository::cooldown),
    (&repository::TOOLING, repository::tooling),
];

/// Checks that state github.com facts (retired hosted-runner images, service shutdowns, the
/// runner's Node.js) and are skipped for another `--gh-hostname`. `removed-feature` keeps its
/// host-independent part in `REGISTRY`.
const HOSTED: &[(&Rule, Detect, Reads)] = &[
    (
        &deprecation::REMOVED,
        deprecation::removed_hosted,
        Reads::Steps,
    ),
    (
        &deprecation::RUNTIME,
        deprecation::retired_runtime,
        Reads::Steps,
    ),
    (
        &deprecation::RUNTIME,
        deprecation::retired_action_runtime,
        Reads::Metadata,
    ),
];

/// Every rule once, with the scope it reads: workflow and action files, the repository, or the
/// GitHub API.
pub fn all() -> impl Iterator<Item = (&'static Rule, &'static str)> {
    let files = REGISTRY
        .iter()
        .chain(HOSTED)
        .map(|(rule, ..)| *rule)
        .chain(CONFIGURABLE.iter().map(|(rule, ..)| *rule))
        .map(|rule| (rule, "workflow"));
    let mut seen: Vec<&str> = Vec::new();
    files
        .chain(ONLINE.iter().map(|(rule, ..)| (*rule, "online")))
        .chain(REPOSITORY.iter().map(|(rule, _)| (*rule, "repository")))
        .chain([(&online::SETTINGS, "repository")])
        .filter(move |(rule, _)| {
            let new = !seen.contains(&rule.id);
            seen.push(rule.id);
            new
        })
}

/// What a rule checks in action metadata files: `steps` (a composite action's `runs.steps` and
/// `uses` lines, or text anywhere in the file) and `metadata` (`runs` and the required keys).
pub fn action_checks(rule: &Rule) -> Vec<&'static str> {
    let reads: Vec<Reads> = REGISTRY
        .iter()
        .chain(HOSTED)
        .chain(OFFLINE)
        .map(|(row, _, reads)| (*row, *reads))
        .chain(NAMED.iter().map(|(row, _, reads)| (*row, *reads)))
        .chain(CONFIGURABLE.iter().map(|(row, _, reads)| (*row, *reads)))
        .chain(ONLINE.iter().map(|(row, _, reads)| (*row, *reads)))
        .filter(|(row, _)| row.id == rule.id)
        .map(|(_, reads)| reads)
        .collect();
    [(Reads::Steps, "steps"), (Reads::Metadata, "metadata")]
        .into_iter()
        .filter(|(check, _)| reads.contains(check))
        .map(|(_, name)| name)
        .collect()
}

/// Config IDs that no crosgha rule or equivalent answers to.
pub fn unknown_ids(config: &crate::config::Config) -> Vec<String> {
    config
        .ids()
        .filter(|id| !all().any(|(rule, _)| rule.answers_to(id)))
        .map(str::to_owned)
        .collect()
}

/// Config IDs of known rules whose `config` block no crosgha rule reads.
pub fn unused_options(config: &crate::config::Config) -> Vec<String> {
    let readers: Vec<&Rule> = CONFIGURABLE
        .iter()
        .map(|(rule, ..)| *rule)
        .chain(ONLINE.iter().map(|(rule, ..)| *rule))
        .chain(REPOSITORY.iter().map(|(rule, _)| *rule))
        .filter(|rule| rule.configurable)
        .collect();
    config
        .configured_ids()
        .filter(|id| {
            all().any(|(rule, _)| rule.answers_to(id))
                && !readers.iter().any(|rule| rule.answers_to(id))
        })
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
    for (rule, detect) in REPOSITORY {
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

/// Runs every enabled rule that reads `workflow`'s kind of file on the file stored in `file` (a base
/// name, for config ignores). Any parsed workflow or action can come here, whatever its source.
pub fn detect_all(
    workflow: &Workflow,
    settings: &Settings,
    file: &str,
) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    let runs =
        |rule: &'static Rule, reads: &Reads| reads.covers(workflow.kind) && enabled(rule, settings);
    let hosted = if settings.hosted { HOSTED } else { &[] };
    for (rule, detect, reads) in REGISTRY.iter().chain(hosted) {
        if runs(rule, reads) {
            findings.extend(detect(workflow)?);
        }
    }
    for (rule, detect, reads) in NAMED {
        if runs(rule, reads) {
            findings.extend(detect(workflow, file)?);
        }
    }
    for (rule, detect, reads) in CONFIGURABLE {
        if runs(rule, reads) {
            findings.extend(detect(workflow, settings.config.options(rule))?);
        }
    }
    if let Some(github) = &settings.github {
        for (rule, detect, reads) in ONLINE {
            if runs(rule, reads) {
                findings.extend(detect(workflow, github, settings.config.options(rule))?);
            }
        }
    } else {
        for (rule, detect, reads) in OFFLINE {
            if runs(rule, reads) {
                findings.extend(detect(workflow)?);
            }
        }
    }
    // Few workflows carry an ignore comment, and a comment is a slice of the source.
    let source = workflow.document.source();
    if settings.ignores && IGNORE_MARKERS.iter().any(|marker| source.contains(marker)) {
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

const IGNORE_MARKERS: [&str; 2] = ["crosgha: ignore[", "zizmor: ignore["];

fn ignore_list(comment: &str) -> Vec<&str> {
    IGNORE_MARKERS
        .iter()
        .filter_map(|marker| comment.split_once(marker))
        .filter_map(|(_, rest)| rest.split_once(']'))
        .flat_map(|(ids, _)| ids.split(',').map(str::trim))
        .collect()
}
