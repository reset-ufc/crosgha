use std::collections::BTreeSet;

use yaml_serde::Value;
use yamlpath::Route;

use super::{
    Finding, Rule,
    common::{self, EXTERNAL_TRIGGERS},
    expression,
};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GitHub recommends granting GITHUB_TOKEN the least access each job needs:
// https://docs.github.com/en/actions/reference/security/secure-use#using-secrets
// GHAST PERM-1 flags permissions declared only at the workflow level.
pub const EXCESSIVE: Rule = Rule {
    id: "excessive-permissions",
    summary: "GITHUB_TOKEN must not get read-all, write-all, or workflow-level write scopes that several jobs inherit.",
    tag: "excessive-permissions",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "excessive-permissions")],
    configurable: false,
    relational: false,
};

// ghalint 005 (workflow-level secrets in env) and zizmor `overprovisioned-secrets`
// (`toJSON(secrets)`, dynamic `secrets[...]`).
pub const SECRETS_EXPOSURE: Rule = Rule {
    id: "secrets-exposure",
    summary: "Secrets must not be exposed to every step or read in bulk.",
    tag: "secrets-exposure",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "overprovisioned-secrets")],
    configurable: false,
    relational: false,
};

// actionlint `credentials` and zizmor `hardcoded-container-credentials`, plus token formats
// GitHub and AWS publish for secret scanning.
pub const HARDCODED: Rule = Rule {
    id: "hardcoded-credentials",
    summary: "Passwords and access tokens must not be written in workflows.",
    tag: "hardcoded-credentials",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[
        ("zizmor", "hardcoded-container-credentials"),
        ("actionlint", "credentials"),
    ],
    configurable: false,
    relational: false,
};

// Trajan `oidc-token-not-narrowed`: `id-token: write` granted beyond the job that deploys,
// or on a trigger outsiders control.
// https://docs.github.com/en/actions/concepts/security/openid-connect
pub const OIDC: Rule = Rule {
    id: "oidc-misconfig",
    summary: "`id-token: write` must be scoped to the job that needs it.",
    tag: "oidc-misconfig",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    equivalents: &[],
    configurable: false,
    relational: false,
};

fn permissions_route() -> Route<'static> {
    Route::default().with_key("permissions")
}

/// Jobs that get the workflow-level `permissions`: a job's own block replaces them. With one such
/// job, workflow-level scopes are as narrow as job-level ones, unless it calls a reusable workflow,
/// whose jobs all inherit them; that call counts as two.
fn inheriting_jobs(workflow: &Workflow) -> usize {
    common::jobs(workflow)
        .iter()
        .filter(|job| job.get("permissions").is_none())
        .map(|job| if job.get("uses").is_some() { 2 } else { 1 })
        .sum()
}

/// Why a `permissions` shorthand is excessive: it grants every scope instead of the ones a job uses.
fn blanket(scope: &str) -> Option<&'static str> {
    match scope {
        "write-all" => Some("`write-all` grants every GITHUB_TOKEN scope"),
        "read-all" => Some("`read-all` grants read access to every GITHUB_TOKEN scope"),
        _ => None,
    }
}

pub fn excessive(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    let scopes = workflow.value.get("permissions");
    let wide_writes = scopes.and_then(Value::as_mapping).is_some_and(|scopes| {
        scopes
            .values()
            .any(|access| access.as_str() == Some("write"))
    }) && inheriting_jobs(workflow) > 1;
    let message = scopes
        .and_then(Value::as_str)
        .and_then(blanket)
        .or(wide_writes.then_some("write scopes at the workflow level reach every job"));
    if let Some(message) = message {
        findings.push(common::finding(
            workflow,
            &EXCESSIVE,
            permissions_route(),
            "workflow",
            message,
        )?);
    }
    for job in common::jobs(workflow) {
        if let Some(message) = job
            .get("permissions")
            .and_then(Value::as_str)
            .and_then(blanket)
        {
            findings.push(common::finding(
                workflow,
                &EXCESSIVE,
                job.route.with_key("permissions"),
                job.location(),
                message,
            )?);
        }
    }
    Ok(findings)
}

pub fn secrets_exposure(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    if let Some(env) = workflow.value.get("env").and_then(Value::as_mapping) {
        for (name, value) in env {
            let (Some(name), Some(text)) = (name.as_str(), value.as_str()) else {
                continue;
            };
            if expression::expressions(text)
                .iter()
                .any(|expression| expression.contains("secrets."))
            {
                findings.push(common::finding(
                    workflow,
                    &SECRETS_EXPOSURE,
                    Route::default().with_key("env").with_key(name.to_owned()),
                    "workflow",
                    "workflow-level env exposes a secret to every step",
                )?);
            }
        }
    }

    let mut seen = BTreeSet::new();
    let mut flagged = Vec::new();
    common::strings(&workflow.value, &Route::default(), &mut |route, text| {
        let broad = expression::expressions(text).into_iter().any(|expression| {
            let compact: String = expression
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>()
                .to_ascii_lowercase();
            compact.contains("tojson(secrets)")
                || compact
                    .split("secrets[")
                    .skip(1)
                    .any(|index| !index.starts_with(['\'', '"']))
        });
        if broad && seen.insert(format!("{route:?}")) {
            flagged.push(route.clone());
        }
    });
    for route in flagged {
        findings.push(common::finding(
            workflow,
            &SECRETS_EXPOSURE,
            route,
            "workflow",
            "expression reads every secret or a secret chosen at runtime",
        )?);
    }
    Ok(findings)
}

pub fn hardcoded(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut flagged: Vec<(Route<'static>, &str)> = Vec::new();
    for job in common::jobs(workflow) {
        let mut containers = vec![(job.route.with_key("container"), job.get("container"))];
        if let Some(services) = job.get("services").and_then(Value::as_mapping) {
            for (name, service) in services {
                if let Some(name) = name.as_str() {
                    containers.push((
                        job.route.with_key("services").with_key(name.to_owned()),
                        Some(service),
                    ));
                }
            }
        }
        for (route, container) in containers {
            let password = container
                .and_then(|container| container.get("credentials"))
                .and_then(|credentials| credentials.get("password"))
                .and_then(Value::as_str);
            if password.is_some_and(|password| !password.contains("${{")) {
                flagged.push((
                    route.with_key("credentials").with_key("password"),
                    "container registry password is written in the workflow",
                ));
            }
        }
    }
    common::strings(&workflow.value, &Route::default(), &mut |route, text| {
        if contains_token(text) {
            flagged.push((route.clone(), "workflow contains a literal access token"));
        }
    });

    let mut seen = BTreeSet::new();
    let mut findings = Vec::new();
    for (route, message) in flagged {
        if seen.insert(format!("{route:?}")) {
            findings.push(common::finding(
                workflow, &HARDCODED, route, "workflow", message,
            )?);
        }
    }
    Ok(findings)
}

/// Token prefixes documented by GitHub and AWS:
/// https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/about-authentication-to-github#githubs-token-formats
/// https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_identifiers.html#identifiers-unique-ids
fn contains_token(text: &str) -> bool {
    let run_after = |prefix: &str, minimum: usize, allowed: fn(u8) -> bool| {
        text.match_indices(prefix).any(|(index, _)| {
            let boundary = index == 0 || !text.as_bytes()[index - 1].is_ascii_alphanumeric();
            let length = text.as_bytes()[index + prefix.len()..]
                .iter()
                .take_while(|byte| allowed(**byte))
                .count();
            boundary && length >= minimum
        })
    };
    ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"]
        .iter()
        .any(|prefix| run_after(prefix, 36, |byte| byte.is_ascii_alphanumeric()))
        || run_after("github_pat_", 22, |byte| {
            byte.is_ascii_alphanumeric() || byte == b'_'
        })
        || text.match_indices("AKIA").any(|(index, _)| {
            let boundary = index == 0 || !text.as_bytes()[index - 1].is_ascii_alphanumeric();
            let rest = &text.as_bytes()[index + 4..];
            boundary
                && rest.len() >= 16
                && rest[..16]
                    .iter()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
                && rest
                    .get(16)
                    .is_none_or(|byte| !byte.is_ascii_alphanumeric())
        })
        || (text.contains("-----BEGIN") && text.contains("PRIVATE KEY-----"))
}

pub fn oidc(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    let grants = |scopes: Option<&Value>| {
        scopes
            .and_then(|scopes| scopes.get("id-token"))
            .and_then(Value::as_str)
            == Some("write")
    };
    let external = common::has_trigger(workflow, EXTERNAL_TRIGGERS);
    if grants(workflow.value.get("permissions")) {
        let message = if inheriting_jobs(workflow) > 1 {
            Some("every job can mint an OIDC token")
        } else {
            external.then_some("OIDC token is mintable on a trigger outsiders control")
        };
        if let Some(message) = message {
            findings.push(common::finding(
                workflow,
                &OIDC,
                permissions_route().with_key("id-token"),
                "workflow",
                message,
            )?);
        }
    }
    if external {
        for job in common::jobs(workflow) {
            if grants(job.get("permissions")) {
                findings.push(common::finding(
                    workflow,
                    &OIDC,
                    job.route.with_key("permissions").with_key("id-token"),
                    job.location(),
                    "OIDC token is mintable on a trigger outsiders control",
                )?);
            }
        }
    }
    Ok(findings)
}
