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
    /// Index of a rejected fix's finding in `FixOutcome::remaining`, whose lines the applied
    /// fixes may have moved.
    pub remaining: Option<usize>,
}

/// What every patch is checked against: the findings of the file as read, and how they were found.
struct Context<'a> {
    original: &'a [Finding],
    settings: &'a Settings,
    file: &'a str,
}

/// Applies every proposal that passes validation. A rejected proposal leaves the source as it
/// was, so its finding stays among the remaining ones and the other proposals still run.
pub fn apply(
    workflow: Workflow,
    findings: &[Finding],
    settings: &Settings,
    file: &str,
) -> Result<FixOutcome, String> {
    let context = Context {
        original: findings,
        settings,
        file,
    };
    let normalized = LineBreaks::normalize(workflow.document.source())
        .and_then(|(lf, breaks)| Some((Workflow::parse(lf).ok()?, breaks)));
    let (mut current, mut breaks) = match normalized {
        Some((parsed, breaks)) => (parsed, Some(breaks)),
        None => (workflow, None),
    };
    // Findings of `current`. The caller's findings describe it until the source is normalized;
    // after that, each accepted patch's validation has already found them.
    let mut known: Option<Vec<Finding>> = None;
    let mut attempts = Vec::new();
    for (index, finding) in findings.iter().enumerate() {
        let Some(proposal) = &finding.fix else {
            continue;
        };
        if breaks.is_some() && known.is_none() {
            match rules::detect_all(&current, settings, file) {
                Ok(found) => known = Some(found),
                Err(reason) => {
                    attempts.push(Attempt {
                        finding: index,
                        rejected: Some(reason),
                        remaining: None,
                    });
                    continue;
                }
            }
        }
        let before = known.as_deref().unwrap_or(findings);
        let rejected = match patch(&current, finding, proposal, before, &context) {
            Ok((next, after)) => {
                if let Some(breaks) = &mut breaks {
                    breaks.carry(current.document.source(), next.document.source());
                }
                current = next;
                known = Some(after);
                None
            }
            Err(reason) => Some(reason),
        };
        attempts.push(Attempt {
            finding: index,
            rejected,
            remaining: None,
        });
    }
    let remaining = match known {
        Some(found) => found,
        None => rules::detect_all(&current, settings, file)?,
    };
    let mut taken = vec![false; remaining.len()];
    for attempt in attempts
        .iter_mut()
        .filter(|attempt| attempt.rejected.is_some())
    {
        let original = &findings[attempt.finding];
        attempt.remaining = (0..remaining.len())
            .find(|&index| !taken[index] && same_target(&remaining[index], original));
        if let Some(index) = attempt.remaining {
            taken[index] = true;
        }
    }
    let source = match &breaks {
        None => current.document.source().to_owned(),
        Some(breaks) => breaks.restore(current.document.source()),
    };
    Ok(FixOutcome {
        source,
        attempts,
        remaining,
    })
}

/// yamlpatch writes `\n` line breaks and ends the file with one, so a file with `\r\n` breaks or
/// no final break is patched in that form and each line gets its own break back. YAML normalizes
/// line breaks in scalars, so neither conversion changes a value.
struct LineBreaks {
    /// The break (`\r\n` or `\n`) ending each line; a last line without one takes the break of
    /// the line before it.
    ends: Vec<&'static str>,
    unterminated: bool,
}

impl LineBreaks {
    /// `source` in yamlpatch's form and the breaks to restore, or `None` when it already is.
    fn normalize(source: &str) -> Option<(String, Self)> {
        let unterminated = !source.ends_with('\n');
        if !unterminated && !source.contains("\r\n") {
            return None;
        }
        let mut lf = source.replace("\r\n", "\n");
        if unterminated {
            lf.push('\n');
        }
        let mut ends: Vec<&'static str> = Vec::new();
        for line in source.split_inclusive('\n') {
            ends.push(match line {
                _ if line.ends_with("\r\n") => "\r\n",
                _ if line.ends_with('\n') => "\n",
                _ => ends.last().copied().unwrap_or("\n"),
            });
        }
        Some((lf, Self { ends, unterminated }))
    }

    /// Moves the breaks of `before`'s lines onto `after`'s, which differ in one region: lines
    /// around it keep their break, a changed line keeps the break of the line it replaces, and an
    /// added line takes the break of the line before it.
    fn carry(&mut self, before: &str, after: &str) {
        let old: Vec<&str> = before.split_inclusive('\n').collect();
        let new: Vec<&str> = after.split_inclusive('\n').collect();
        let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let suffix = old[prefix..]
            .iter()
            .rev()
            .zip(new[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let replaced = old.len() - prefix - suffix;
        let mut carried = self.ends[..prefix].to_vec();
        for index in 0..new.len() - prefix - suffix {
            let end = match index < replaced {
                true => self.ends[prefix + index],
                false => carried
                    .last()
                    .or(self.ends.get(prefix))
                    .copied()
                    .unwrap_or("\n"),
            };
            carried.push(end);
        }
        carried.extend_from_slice(&self.ends[old.len() - suffix..]);
        self.ends = carried;
    }

    /// `lf`, the patched source, with the file's own breaks.
    fn restore(&self, lf: &str) -> String {
        let mut restored = String::with_capacity(lf.len() + self.ends.len());
        for (line, end) in lf.split_inclusive('\n').zip(&self.ends) {
            restored.push_str(line.strip_suffix('\n').unwrap_or(line));
            restored.push_str(end);
        }
        if self.unterminated {
            restored.truncate(restored.trim_end_matches(['\r', '\n']).len());
        }
        restored
    }
}

/// The patched workflow and its findings, once the patch passes validation. `before` holds the
/// findings of `current`.
fn patch(
    current: &Workflow,
    finding: &Finding,
    proposal: &ProposedFix,
    before: &[Finding],
    context: &Context,
) -> Result<(Workflow, Vec<Finding>), String> {
    let patched = apply_yaml_patches(&current.document, std::slice::from_ref(&proposal.patch))
        .map_err(|error| error.to_string())?;
    let mut source = patched.source().to_owned();
    if matches!(proposal.change, ChangeKind::Add)
        && let Some(moved) = after_trailing_whitespace(current.document.source(), &source)
    {
        source = moved;
    }
    let next = Workflow::parse(source)?;
    let after = rules::detect_all(&next, context.settings, context.file)?;
    compare_findings(finding, before, &after, context.original)?;
    validate_change(current, &next, proposal)?;
    Ok((next, after))
}

/// The target must disappear and no other finding may change, except that findings of
/// `relational` rules may disappear, or come back when the file as read had them.
fn compare_findings(
    target: &Finding,
    old: &[Finding],
    new: &[Finding],
    original: &[Finding],
) -> Result<(), String> {
    let compared = |findings: &[Finding]| {
        findings
            .iter()
            .filter(|finding| !finding.rule.relational)
            .count()
    };
    if new.iter().any(|finding| same_target(finding, target)) || compared(new) + 1 != compared(old)
    {
        return Err("fix did not remove exactly the targeted finding".to_owned());
    }
    let known = |finding: &Finding| {
        let restored = match finding.rule.relational {
            true => original,
            false => &[],
        };
        old.iter()
            .chain(restored)
            .any(|old| same_target(old, finding))
    };
    if !new.iter().all(known) {
        return Err("fix introduced a new finding".to_owned());
    }
    Ok(())
}

fn validate_change(
    before: &Workflow,
    after: &Workflow,
    proposal: &ProposedFix,
) -> Result<(), String> {
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
    Ok(())
}

/// yamlpatch adds block lines right after the last value, before that line's trailing spaces,
/// which then end the added line instead. This moves such an insertion to the next line
/// boundary, so the existing line keeps its bytes.
fn after_trailing_whitespace(before: &str, after: &str) -> Option<String> {
    let length = after.len().checked_sub(before.len())?;
    let at = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let rest = before.get(at..)?;
    let lines = after.get(at..at + length)?.strip_prefix('\n')?;
    let line_end = rest.find('\n')?;
    let trailing = rest[..line_end]
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t'));
    if line_end == 0 || !trailing || after[at + length..] != *rest {
        return None;
    }
    let boundary = at + line_end + 1;
    Some(format!(
        "{}{lines}\n{}",
        &before[..boundary],
        &before[boundary..]
    ))
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
    // When the insertion repeats its surroundings, it can start anywhere in `lowest..=highest`.
    let lowest = (old.len() - suffix).max(node.end.saturating_sub(length));
    let highest = prefix.min(node.start);
    if prefix + suffix < old.len() || highest < lowest {
        return false;
    }
    // Added lines must be whole lines, so every line around them keeps its bytes.
    let start = match new[highest..highest + length].contains(&b'\n') {
        false => highest,
        true => match (lowest..=highest)
            .find(|&at| (at == 0 || new[at - 1] == b'\n') && new[at + length - 1] == b'\n')
        {
            Some(at) => at,
            None => return false,
        },
    };
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
