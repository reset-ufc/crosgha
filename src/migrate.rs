//! `crosgha migrate --from <tool>`: swaps another scanner's Action step for the crosgha
//! Action, the way `@oxlint/migrate` moves ESLint users over. crosgha's Action accepts
//! zizmor-action's inputs, so zizmor steps keep their `with:` block; actionlint steps drop
//! theirs because none of its inputs apply.

use std::{ops::Range, path::Path};

use clap::ValueEnum;

use crate::{filesystem, rules::common, workflow::Workflow};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Tool {
    Zizmor,
    Actionlint,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Zizmor => "zizmor",
            Self::Actionlint => "actionlint",
        }
    }

    /// `uses:` prefixes of the tool's published Actions and container image.
    fn actions(self) -> &'static [&'static str] {
        match self {
            Self::Zizmor => &["zizmorcore/zizmor-action@"],
            Self::Actionlint => &[
                "raven-actions/actionlint@",
                "reviewdog/action-actionlint@",
                "rhysd/actionlint@",
                "docker://rhysd/actionlint",
            ],
        }
    }

    fn keeps_inputs(self) -> bool {
        self == Self::Zizmor
    }
}

pub struct Summary {
    pub steps: usize,
    pub files: usize,
    /// Shell steps that run the tool directly and need a manual change.
    pub notes: Vec<String>,
}

pub fn run(
    path: &Path,
    tool: Tool,
    action: &str,
    version: Option<&str>,
) -> Result<Summary, String> {
    let replacement = match version {
        Some(version) => format!("{action} # {version}"),
        None => action.to_owned(),
    };
    let mut summary = Summary {
        steps: 0,
        files: 0,
        notes: Vec::new(),
    };
    let mut rewrites = Vec::new();
    for file in filesystem::discover(path)? {
        let source = filesystem::read(&file, true)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        let workflow =
            Workflow::parse(source).map_err(|error| format!("{}: {error}", file.display()))?;
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        for job in common::jobs(&workflow) {
            for step in common::steps(&job) {
                if step.uses().is_some_and(|uses| {
                    tool.actions().iter().any(|prefix| uses.starts_with(prefix))
                }) {
                    edits.push((
                        value_to_line_end(&workflow, &step.key("uses"))?,
                        replacement.clone(),
                    ));
                    if !tool.keeps_inputs() && step.value.get("with").is_some() {
                        let span = workflow
                            .document
                            .removal_span(&step.key("with"))
                            .map_err(|error| error.to_string())?;
                        edits.push((span, String::new()));
                    }
                }
                if step
                    .run()
                    .is_some_and(|script| mentions(script, tool.name()))
                {
                    summary.notes.push(format!(
                        "{}:{}: {}: runs {} from a shell; replace it with the crosgha Action by hand",
                        file.display(),
                        common::line(&workflow, &step.key("run"))?,
                        step.location,
                        tool.name()
                    ));
                }
            }
        }
        if edits.is_empty() {
            continue;
        }
        summary.steps += edits.iter().filter(|(_, text)| !text.is_empty()).count();
        summary.files += 1;
        let mut migrated = workflow.document.source().to_owned();
        edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, text) in edits {
            migrated.replace_range(range, &text);
        }
        Workflow::parse(migrated.clone()).map_err(|error| {
            format!(
                "{}: migration produced invalid YAML: {error}",
                file.display()
            )
        })?;
        rewrites.push((file, migrated));
    }
    // Every file is migrated in memory before any is written.
    for (file, migrated) in rewrites {
        filesystem::write_verified(&file, &migrated)
            .map_err(|error| format!("{}: {error}", file.display()))?;
    }
    Ok(summary)
}

/// The `uses` value through the end of its line, so the old `# vX.Y.Z` comment goes with it.
fn value_to_line_end(workflow: &Workflow, route: &yamlpath::Route) -> Result<Range<usize>, String> {
    let feature = workflow
        .document
        .query_exact(route)
        .map_err(|error| error.to_string())?
        .ok_or("`uses` has no value")?;
    let (start, end) = feature.location.byte_span;
    let source = workflow.document.source();
    let line_end = source[end..]
        .find('\n')
        .map_or(source.len(), |offset| end + offset);
    Ok(start..line_end)
}

fn mentions(script: &str, tool: &str) -> bool {
    script
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
        .any(|word| word == tool)
}
