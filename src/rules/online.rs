//! Rules that need the GitHub API. They run only when a token is available and `--offline`
//! is not set, like zizmor's online audits: https://docs.zizmor.sh/usage/#operating-modes

use std::path::Path;

use yamlpath::Route;

use super::{Finding, Located, Options, Rule, common};
use crate::{
    github::GitHub,
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
    version: Option<String>,
}

fn pins(workflow: &Workflow) -> Vec<Pin<'_>> {
    common::all_uses(workflow)
        .into_iter()
        .filter_map(|(route, location, uses)| {
            let reference = common::parse_uses(uses)?;
            let sha = Some(reference.git_ref).filter(|git_ref| {
                git_ref.len() == 40 && git_ref.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
            let version = match sha {
                Some(_) => comment_version(workflow, &route),
                None => Some(reference.git_ref.to_owned()).filter(|git_ref| is_version(git_ref)),
            };
            Some(Pin {
                route,
                location,
                owner: reference.owner,
                repo: reference.repo,
                sha,
                version,
            })
        })
        .collect()
}

/// The first word of the YAML comment on a `uses` line, such as `v4.2.2` in `# v4.2.2`.
fn comment_version(workflow: &Workflow, route: &Route) -> Option<String> {
    let feature = workflow.document.query_pretty(route).ok()?;
    let comments = workflow.document.feature_comments(&feature);
    let text = workflow.document.extract(comments.first()?);
    let word = text.trim_start_matches('#').split_whitespace().next()?;
    is_version(word).then(|| word.to_owned())
}

fn is_version(text: &str) -> bool {
    text.strip_prefix('v')
        .unwrap_or(text)
        .starts_with(|character: char| character.is_ascii_digit())
}

fn numbers(version: &str) -> Vec<u64> {
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
        let Some(version) = &pin.version else {
            continue;
        };
        let path = format!(
            "/advisories?ecosystem=actions&affects={}/{}@{}",
            pin.owner,
            pin.repo,
            version.trim_start_matches('v')
        );
        let advisories = github.get(&path)?.unwrap_or_default();
        let ids: Vec<&str> = advisories
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|advisory| advisory.get("ghsa_id")?.as_str())
            .filter(|id| !allowed.contains(id))
            .collect();
        if !ids.is_empty() {
            findings.push(common::finding(
                workflow,
                &VULNERABLE,
                pin.route,
                pin.location,
                format!("{version} is affected by {}", ids.join(", ")),
            )?);
        }
    }
    Ok(findings)
}

pub fn outdated(
    workflow: &Workflow,
    github: &GitHub,
    _options: Options,
) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for pin in pins(workflow) {
        let Some(version) = &pin.version else {
            continue;
        };
        let latest = github.get(&format!(
            "/repos/{}/{}/releases/latest",
            pin.owner, pin.repo
        ))?;
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
        let (Some(sha), Some(version)) = (pin.sha, &pin.version) else {
            continue;
        };
        let tagged = github.get(&format!(
            "/repos/{}/{}/commits/{version}",
            pin.owner, pin.repo
        ))?;
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
        let Some(repository) = github.get(&format!("/repos/{}/{}", pin.owner, pin.repo))? else {
            continue;
        };
        let Some(branch) = repository
            .get("default_branch")
            .and_then(|branch| branch.as_str())
        else {
            continue;
        };
        // ponytail: Checks the default branch and the first page of tags; walk every
        // branch when release branches produce false positives.
        let comparison = github.get(&format!(
            "/repos/{}/{}/compare/{branch}...{sha}",
            pin.owner, pin.repo
        ))?;
        let on_branch = comparison
            .as_ref()
            .and_then(|comparison| comparison.get("status")?.as_str())
            .is_some_and(|status| matches!(status, "behind" | "identical"));
        let tags = github
            .get(&format!(
                "/repos/{}/{}/tags?per_page=100",
                pin.owner, pin.repo
            ))?
            .unwrap_or_default();
        let tagged = tags.as_array().into_iter().flatten().any(|tag| {
            tag.get("commit")
                .and_then(|commit| commit.get("sha")?.as_str())
                .is_some_and(|tagged| tagged.eq_ignore_ascii_case(sha))
        });
        if !on_branch && !tagged {
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
        let repository = github.get(&format!("/repos/{}/{}", pin.owner, pin.repo))?;
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

pub fn settings(github: &GitHub, repository: &str, path: &Path) -> Result<Vec<Located>, String> {
    let Some(workflow) =
        github.get(&format!("/repos/{repository}/actions/permissions/workflow"))?
    else {
        return Ok(Vec::new());
    };
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
