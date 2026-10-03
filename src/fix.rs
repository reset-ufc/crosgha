use yaml_serde::Value;
use yamlpatch::{Op, apply_yaml_patches};

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
    // yamlpatch writes `\n` line breaks and ends the file with one, so the file is patched in
    // that form and converted back. YAML normalizes line breaks in scalars, so neither
    // conversion changes a value.
    let source = workflow.document.source();
    let crlf = source.contains("\r\n") && !source.replace("\r\n", "\n").contains('\r');
    let unterminated = !source.ends_with('\n');
    let mut normalized = match crlf {
        true => source.replace("\r\n", "\n"),
        false => source.to_owned(),
    };
    if unterminated {
        normalized.push('\n');
    }
    let (mut current, crlf, unterminated) = match crlf || unterminated {
        true => match Workflow::parse(normalized) {
            Ok(normalized) => (normalized, crlf, unterminated),
            Err(_) => (workflow, false, false),
        },
        false => (workflow, false, false),
    };
    // Findings of `current`. The caller's findings describe it until the source is normalized;
    // after that, each accepted patch's validation has already found them.
    let mut known: Option<Vec<Finding>> = None;
    let normalized = crlf || unterminated;
    let mut attempts = Vec::new();
    for (index, finding) in findings.iter().enumerate() {
        let Some(proposal) = &finding.fix else {
            continue;
        };
        if normalized && known.is_none() {
            match rules::detect_all(&current, settings, file) {
                Ok(found) => known = Some(found),
                Err(reason) => {
                    attempts.push(Attempt {
                        finding: index,
                        rejected: Some(reason),
                    });
                    continue;
                }
            }
        }
        let before = known.as_deref().unwrap_or(findings);
        let rejected = match patch(&current, finding, proposal, before, settings, file) {
            Ok((next, after)) => {
                current = next;
                known = Some(after);
                None
            }
            Err(reason) => Some(reason),
        };
        attempts.push(Attempt {
            finding: index,
            rejected,
        });
    }
    let remaining = match known {
        Some(found) => found,
        None => rules::detect_all(&current, settings, file)?,
    };
    let mut source = current.document.source();
    if unterminated {
        source = source.strip_suffix('\n').unwrap_or(source);
    }
    Ok(FixOutcome {
        source: match crlf {
            true => source.replace('\n', "\r\n"),
            false => source.to_owned(),
        },
        attempts,
        remaining,
    })
}

/// The patched workflow and its findings, once the patch passes validation.
fn patch(
    current: &Workflow,
    finding: &Finding,
    proposal: &ProposedFix,
    before: &[Finding],
    settings: &Settings,
    file: &str,
) -> Result<(Workflow, Vec<Finding>), String> {
    let patched = apply_yaml_patches(&current.document, std::slice::from_ref(&proposal.patch))
        .map_err(|error| error.to_string())?;
    let next = Workflow::parse(patched.source().to_owned())?;
    let after = validate_change(current, &next, finding, proposal, before, settings, file)?;
    Ok((next, after))
}

fn validate_change(
    before: &Workflow,
    after: &Workflow,
    target: &Finding,
    proposal: &ProposedFix,
    old_findings: &[Finding],
    settings: &Settings,
    file: &str,
) -> Result<Vec<Finding>, String> {
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
            let Op::Add { key, value } = &proposal.patch.operation else {
                return Err("an add fix must use an add patch".to_owned());
            };
            let start = after
                .document
                .query_key_only(route)
                .map_err(|error| error.to_string())?
                .location
                .byte_span
                .0;
            let end = after
                .document
                .query_pretty(route)
                .map_err(|error| error.to_string())?
                .location
                .byte_span
                .1;
            let source = after.document.source();
            inserts_only(
                before.document.source(),
                source,
                start..content_end(source, start, end),
            ) && adds_one(&before.value, &after.value, key, value)
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
    Ok(new_findings)
}

/// `end` without the blank and comment lines that close `source[start..end]`: the parser counts
/// comments after a block mapping as part of its last node.
fn content_end(source: &str, start: usize, mut end: usize) -> usize {
    loop {
        let trimmed = start + source[start..end].trim_end().len();
        let line = source[start..trimmed]
            .rfind('\n')
            .map_or(start, |at| start + at + 1);
        if line == start || !source[line..trimmed].trim_start().starts_with('#') {
            return trimmed;
        }
        end = line;
    }
}

/// Whether `after` is `before` plus one contiguous insertion made of `after[node]` and only the
/// spaces, line breaks, and single comma that separate it from its neighbors.
fn inserts_only(before: &str, after: &str, node: std::ops::Range<usize>) -> bool {
    let (old, new) = (before.as_bytes(), after.as_bytes());
    let Some(length) = new.len().checked_sub(old.len()) else {
        return false;
    };
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    // When the insertion repeats its surroundings, it can start anywhere in `lowest..=prefix`.
    let lowest = (old.len() - suffix).max(node.end.saturating_sub(length));
    let start = prefix.min(node.start);
    if prefix + suffix < old.len() || start < lowest {
        return false;
    }
    let mut commas = 0;
    new[start..node.start]
        .iter()
        .chain(&new[node.end..start + length])
        .all(|byte| match byte {
            b' ' | b'\n' => true,
            b',' => {
                commas += 1;
                commas == 1
            }
            _ => false,
        })
}

/// Whether `after` equals `before` except for one mapping that gained `key: value`. This catches
/// separator whitespace that re-indents a neighbor, which `inserts_only` allows.
fn adds_one(before: &Value, after: &Value, key: &str, value: &Value) -> bool {
    match (before, after) {
        (Value::Mapping(old), Value::Mapping(new)) => {
            if new.len() == old.len() + 1 && !old.contains_key(key) && new.get(key) == Some(value) {
                return old.iter().all(|(name, item)| new.get(name) == Some(item));
            }
            new.len() == old.len()
                && one_adds(
                    old.iter().map(|(name, item)| (item, new.get(name))),
                    key,
                    value,
                )
        }
        (Value::Sequence(old), Value::Sequence(new)) => {
            new.len() == old.len() && one_adds(old.iter().zip(new.iter().map(Some)), key, value)
        }
        _ => false,
    }
}

fn one_adds<'a>(
    pairs: impl Iterator<Item = (&'a Value, Option<&'a Value>)>,
    key: &str,
    value: &Value,
) -> bool {
    let mut changed = 0;
    for (old, new) in pairs {
        match new {
            Some(new) if new == old => {}
            Some(new) if adds_one(old, new, key, value) => changed += 1,
            _ => return false,
        }
    }
    changed == 1
}

fn same_target(left: &Finding, right: &Finding) -> bool {
    left.rule.id == right.rule.id && left.location == right.location
}
