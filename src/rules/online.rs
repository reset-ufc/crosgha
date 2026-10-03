//! Rules that need the GitHub API. They run only when a token is available and `--offline`
//! is not set, like zizmor's online audits: https://docs.zizmor.sh/usage/#operating-modes

use std::{ops::ControlFlow, path::Path, sync::PoisonError};

use yamlpath::Route;

use super::{Finding, Located, Options, Rule, common};
use crate::{
    github::{ApiError, GitHub},
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GitHub Advisory Database, `actions` ecosystem:
// https://docs.github.com/en/rest/security-advisories/global-advisories
pub const VULNERABLE: Rule = Rule {
    id: "known-vulnerable-action",
    summary: "Referenced action versions must not have published advisories.",
    tag: "known-vulnerable-action",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "known-vulnerable-actions")],
    configurable: true,
};

// GHAST TP-1 and CIAnalyser `reportUpdateLag`: the referenced version is older than the
// repository's latest release.
pub const OUTDATED: Rule = Rule {
    id: "outdated-action",
    summary: "Referenced action versions should not lag behind the latest release.",
    tag: "outdated-action",
    severity: Severity::Note,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
};

// pinact `--verify-comment` and zizmor `ref-version-mismatch`: the `# vX.Y.Z` comment next to
// a SHA must name the tag that points at that SHA.
pub const COMMENT_MISMATCH: Rule = Rule {
    id: "version-comment-mismatch",
    summary: "The version comment next to a pinned SHA must name that SHA's tag.",
    tag: "version-comment-mismatch",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    equivalents: &[("zizmor", "ref-version-mismatch")],
    configurable: false,
};

// zizmor `impostor-commit`: a SHA that belongs to a fork, not to the named repository.
pub const IMPOSTOR: Rule = Rule {
    id: "impostor-commit",
    summary: "A pinned SHA must belong to the named repository, not to a fork.",
    tag: "typosquatting/impostor-commit",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "impostor-commit")],
    configurable: false,
};

// zizmor `archived-uses`: archived actions receive no security fixes.
pub const ARCHIVED: Rule = Rule {
    id: "archived-action",
    summary: "Actions from archived repositories receive no security fixes.",
    tag: "untrusted-3rd-party-action",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "archived-uses")],
    configurable: false,
};

// Repository Actions settings that widen every workflow's token:
// https://docs.github.com/en/rest/actions/permissions#get-default-workflow-permissions-for-a-repository
pub const SETTINGS: Rule = Rule {
    id: "repo-org-settings",
    summary: "Repository Actions settings must not grant write tokens or pull request approval by default.",
    tag: "repo-org-settings",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
};

struct Pin<'a> {
    route: Route<'static>,
    location: String,
    owner: &'a str,
    repo: &'a str,
    sha: Option<&'a str>,
    git_ref: &'a str,
}

impl Pin<'_> {
    fn version(&self, workflow: &Workflow) -> Option<String> {
        ref_version(workflow, &self.route, self.git_ref)
    }
}

fn pins(workflow: &Workflow) -> Vec<Pin<'_>> {
    common::all_uses(workflow)
        .into_iter()
        .filter_map(|(route, location, uses)| {
            let reference = common::parse_uses(uses)?;
            let sha = Some(reference.git_ref).filter(|git_ref| {
                git_ref.len() == 40 && git_ref.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
            Some(Pin {
                route,
                location,
                owner: reference.owner,
                repo: reference.repo,
                sha,
                git_ref: reference.git_ref,
            })
        })
        .collect()
}

/// The version a `uses` ref names: the tag itself, or the `# vX.Y.Z` comment after a full SHA.
pub(super) fn ref_version(workflow: &Workflow, route: &Route, git_ref: &str) -> Option<String> {
    if git_ref.len() == 40 && git_ref.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        comment_version(workflow, route)
    } else {
        Some(git_ref.to_owned()).filter(|git_ref| is_version(git_ref))
    }
}

/// The first word of the YAML comment on a `uses` line, such as `v4.2.2` in `# v4.2.2`.
fn comment_version(workflow: &Workflow, route: &Route) -> Option<String> {
    // `Route` has no `Eq`; its debug form names one route.
    let key = format!("{route:?}");
    let mut known = workflow
        .comment_versions
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    known
        .entry(key)
        .or_insert_with(|| {
            let feature = workflow.document.query_pretty(route).ok()?;
            let comments = workflow.document.feature_comments(&feature);
            let text = workflow.document.extract(comments.first()?);
            let word = text.trim_start_matches('#').split_whitespace().next()?;
            is_version(word).then(|| word.to_owned())
        })
        .clone()
}

fn is_version(text: &str) -> bool {
    text.strip_prefix('v')
        .unwrap_or(text)
        .starts_with(|character: char| character.is_ascii_digit())
}

pub(super) fn numbers(version: &str) -> Vec<u64> {
    version
        .trim_start_matches('v')
        .split('.')
        .map_while(|part| {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

pub fn vulnerable(
    workflow: &Workflow,
    github: &GitHub,
    options: Options,
) -> Result<Vec<Finding>, String> {
    // zizmor's `rules.known-vulnerable-actions.config.allow`: advisory IDs to accept.
    let allowed: Vec<&str> = options
        .and_then(|options| options.get("allow"))
        .and_then(|allow| allow.as_sequence())
        .into_iter()
        .flatten()
        .filter_map(|id| id.as_str())
        .collect();
    let mut findings = Vec::new();
    for pin in pins(workflow) {
        let Some(version) = &pin.version(workflow) else {
            continue;
        };
        let Ok(resolved) = release_version(github, &pin, version) else {
            continue;
        };
        let path = format!(
            "/advisories?ecosystem=actions&affects={}/{}@{}",
            pin.owner,
            pin.repo,
            resolved.trim_start_matches('v')
        );
        let Ok(advisories) = github.get(&path) else {
            continue;
        };
        let advisories = advisories.unwrap_or_default();
        let ids: Vec<&str> = advisories
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|advisory| advisory.get("ghsa_id")?.as_str())
            .filter(|id| !allowed.contains(id))
            .collect();
        if !ids.is_empty() {
            let named = if resolved == *version {
                version.clone()
            } else {
                format!("{version} ({resolved})")
            };
            findings.push(common::finding(
                workflow,
                &VULNERABLE,
                pin.route,
                pin.location,
                format!("{named} is affected by {}", ids.join(", ")),
            )?);
        }
    }
    Ok(findings)
}

/// The full release a partial version such as `v4` stands for: the `vX.Y.Z` tag on the same
/// commit. The advisories API reads `@4` as 4.0.0, so `actions/download-artifact@4` matched
/// GHSA-cxww-7g56-2vh6 (`>= 4.0.0, < 4.1.3`) although `v4` points at a fixed 4.x release
/// (`gh api '/advisories?ecosystem=actions&affects=actions/download-artifact@4'`, 2026-10-03).
/// The version stays as written when no full tag shares its commit.
fn release_version(github: &GitHub, pin: &Pin, version: &str) -> Result<String, ApiError> {
    let wanted = numbers(version);
    if wanted.len() >= 3 {
        return Ok(version.to_owned());
    }
    let mut tags: Vec<(String, String)> = Vec::new();
    each_tag(github, pin.owner, pin.repo, |name, sha| {
        tags.push((name.to_owned(), sha.to_ascii_lowercase()));
        ControlFlow::Continue(())
    })?;
    let commit = match pin.sha {
        Some(sha) => Some(sha.to_ascii_lowercase()),
        None => tags
            .iter()
            .find(|(name, _)| name == version)
            .map(|(_, sha)| sha.clone()),
    };
    Ok(tags
        .iter()
        .filter(|(name, sha)| {
            let found = numbers(name);
            Some(sha) == commit.as_ref() && found.len() >= 3 && found.starts_with(&wanted)
        })
        .max_by_key(|(name, _)| numbers(name))
        .map_or_else(|| version.to_owned(), |(name, _)| name.clone()))
}

const TAGS_PER_PAGE: usize = 100;
/// A server that ignores `page` would answer forever; the walk gives up after 10,000 tags.
const MAX_TAG_PAGES: usize = 100;

/// Visits the repository's tags as `(name, commit SHA)` until `visit` breaks, one request per
/// 100 tags: https://docs.github.com/en/rest/repos/repos#list-repository-tags
fn each_tag(
    github: &GitHub,
    owner: &str,
    repo: &str,
    mut visit: impl FnMut(&str, &str) -> ControlFlow<()>,
) -> Result<(), ApiError> {
    let mut previous = None;
    for page in 1..=MAX_TAG_PAGES {
        let path = format!("/repos/{owner}/{repo}/tags?per_page={TAGS_PER_PAGE}&page={page}");
        let answer = github.get(&path)?.unwrap_or_default();
        let tags = answer.as_array().map_or(&[][..], Vec::as_slice);
        if !tags.is_empty() && previous.as_ref() == Some(&answer) {
            let detail = format!("repeats page {}; the server ignores `page`", page - 1);
            return Err(github.reject(&path, &detail));
        }
        for tag in tags {
            let name = tag.get("name").and_then(|name| name.as_str());
            let sha = tag
                .get("commit")
                .and_then(|commit| commit.get("sha")?.as_str());
            if let (Some(name), Some(sha)) = (name, sha)
                && visit(name, sha).is_break()
            {
                return Ok(());
            }
        }
        if tags.len() < TAGS_PER_PAGE {
            return Ok(());
        }
        previous = Some(answer);
    }
    let detail = format!(
        "more than {} tags; the walk stopped",
        MAX_TAG_PAGES * TAGS_PER_PAGE
    );
    Err(github.reject(&format!("/repos/{owner}/{repo}/tags"), &detail))
}

pub fn outdated(
    workflow: &Workflow,
    github: &GitHub,
    _options: Options,
) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for pin in pins(workflow) {
        let Some(version) = &pin.version(workflow) else {
            continue;
        };
        let Ok(latest) = github.get(&format!(
            "/repos/{}/{}/releases/latest",
            pin.owner, pin.repo
        )) else {
            continue;
        };
        let Some(latest) = latest
            .as_ref()
            .and_then(|release| release.get("tag_name")?.as_str())
        else {
            continue;
        };
        let current = numbers(version);
        let newest = numbers(latest);
        // A major-only ref such as `v4` tracks its major line, so compare at its precision.
        if !current.is_empty() && current < newest[..newest.len().min(current.len())].to_vec() {
            findings.push(common::finding(
                workflow,
                &OUTDATED,
                pin.route,
                pin.location,
                format!("{version} is older than the latest release {latest}"),
            )?);
        }
    }
    Ok(findings)
}

pub fn comment_mismatch(
    workflow: &Workflow,
    github: &GitHub,
    _options: Options,
) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for pin in pins(workflow) {
        let (Some(sha), Some(version)) = (pin.sha, pin.version(workflow)) else {
            continue;
        };
        let Ok(tagged) = github.get(&format!(
            "/repos/{}/{}/commits/{version}",
            pin.owner, pin.repo
        )) else {
            continue;
        };
        let message = match tagged
            .as_ref()
            .and_then(|commit| commit.get("sha")?.as_str())
        {
            None => format!("`{version}` does not exist in {}/{}", pin.owner, pin.repo),
            Some(tagged) if !tagged.eq_ignore_ascii_case(sha) => {
                format!("`{version}` points to {tagged}, not to the pinned SHA")
            }
            Some(_) => continue,
        };
        findings.push(common::finding(
            workflow,
            &COMMENT_MISMATCH,
            pin.route,
            pin.location,
            message,
        )?);
    }
    Ok(findings)
}

pub fn impostor(
    workflow: &Workflow,
    github: &GitHub,
    _options: Options,
) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for pin in pins(workflow) {
        let Some(sha) = pin.sha else { continue };
        let Ok(Some(repository)) = github.get(&format!("/repos/{}/{}", pin.owner, pin.repo)) else {
            continue;
        };
        let Some(branch) = repository
            .get("default_branch")
            .and_then(|branch| branch.as_str())
        else {
            continue;
        };
        // ponytail: Checks the default branch and every tag; walk every branch when release
        // branches produce false positives.
        let Ok(comparison) = github.get(&format!(
            "/repos/{}/{}/compare/{branch}...{sha}",
            pin.owner, pin.repo
        )) else {
            continue;
        };
        let on_branch = comparison
            .as_ref()
            .and_then(|comparison| comparison.get("status")?.as_str())
            .is_some_and(|status| matches!(status, "behind" | "identical"));
        if on_branch {
            continue;
        }
        let mut tagged = false;
        let walked = each_tag(github, pin.owner, pin.repo, |_, tagged_sha| {
            tagged = tagged_sha.eq_ignore_ascii_case(sha);
            if tagged {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        if walked.is_ok() && !tagged {
            findings.push(common::finding(
                workflow,
                &IMPOSTOR,
                pin.route,
                pin.location,
                format!(
                    "{sha} is not on {branch} or a tag of {}/{}",
                    pin.owner, pin.repo
                ),
            )?);
        }
    }
    Ok(findings)
}

pub fn archived(
    workflow: &Workflow,
    github: &GitHub,
    _options: Options,
) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for pin in pins(workflow) {
        let Ok(repository) = github.get(&format!("/repos/{}/{}", pin.owner, pin.repo)) else {
            continue;
        };
        if repository
            .as_ref()
            .and_then(|repository| repository.get("archived")?.as_bool())
            == Some(true)
        {
            findings.push(common::finding(
                workflow,
                &ARCHIVED,
                pin.route,
                pin.location,
                format!("{}/{} is archived", pin.owner, pin.repo),
            )?);
        }
    }
    Ok(findings)
}

/// Archived action repositories of the `actions` and `actions-rs` organizations: `archived: true`
/// with an `action.yml`, from the GitHub API on 2026-09-27. `archived-action` reports them without a
/// token; with one, the online check covers every repository and this list is not used.
// ponytail: two organizations only; add others once their archived actions are verified the same way.
const KNOWN_ARCHIVED: &[&str] = &[
    "actions-rs/audit-check",
    "actions-rs/cargo",
    "actions-rs/clippy",
    "actions-rs/clippy-check",
    "actions-rs/components-nightly",
    "actions-rs/grcov",
    "actions-rs/install",
    "actions-rs/tarpaulin",
    "actions-rs/toolchain",
    "actions/create-release",
    "actions/setup-elixir",
    "actions/setup-haskell",
    "actions/setup-ruby",
    "actions/upload-release-asset",
];

pub fn archived_offline(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    pins(workflow)
        .into_iter()
        .filter(|pin| {
            let name = format!("{}/{}", pin.owner, pin.repo).to_ascii_lowercase();
            KNOWN_ARCHIVED.contains(&name.as_str())
        })
        .map(|pin| {
            common::finding(
                workflow,
                &ARCHIVED,
                pin.route,
                pin.location,
                format!("{}/{} is archived", pin.owner, pin.repo),
            )
        })
        .collect()
}

/// A denied read (403, or 404 for a repository the token cannot see) comes back with
/// `ApiError::denied`, so the caller can skip this rule instead of failing the scan.
pub fn settings(github: &GitHub, repository: &str, path: &Path) -> Result<Vec<Located>, ApiError> {
    // The endpoint needs the Administration (read) repository permission
    // (https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens#repository-permissions-for-administration)
    // or a classic token's `repo` scope (https://docs.github.com/en/rest/actions/permissions#get-default-workflow-permissions-for-a-repository).
    // GITHUB_TOKEN cannot be granted Administration, so in Actions this read is usually denied.
    let endpoint = format!("/repos/{repository}/actions/permissions/workflow");
    let workflow = github
        .request(&endpoint)
        .and_then(|found| {
            found.ok_or_else(|| ApiError {
                status: Some(404),
                rate_limited: false,
                message: format!("GitHub API {endpoint}: HTTP 404"),
            })
        })
        .map_err(|mut error| {
            if error.denied() {
                error.message.push_str(
                    "; reading Actions settings needs a token with the Administration (read) \
                     repository permission, which GITHUB_TOKEN cannot be granted",
                );
            }
            error
        })?;
    let mut problems = Vec::new();
    if workflow
        .get("default_workflow_permissions")
        .and_then(|permissions| permissions.as_str())
        == Some("write")
    {
        problems.push("default_workflow_permissions is `write`; use `read`");
    }
    if workflow
        .get("can_approve_pull_request_reviews")
        .and_then(|approve| approve.as_bool())
        == Some(true)
    {
        problems.push("can_approve_pull_request_reviews lets GITHUB_TOKEN approve pull requests");
    }
    Ok(problems
        .into_iter()
        .map(|message| Located {
            path: path.to_path_buf(),
            finding: Finding {
                rule: &SETTINGS,
                severity: SETTINGS.severity,
                message: message.to_owned(),
                location: format!("repository {repository}"),
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 1,
                route: Route::default(),
                fix: None,
                concept: None,
            },
        })
        .collect())
}
