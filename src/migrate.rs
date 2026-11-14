//! `crosgha migrate --from <tool>`: moves a repository from zizmor, actionlint, poutine, or
//! ghalint to crosgha.
//! It rewrites the tool's Action steps, turns shell steps that only install and run the tool into
//! crosgha Action steps, and writes a `crosgha.yml` from the tool's config. The tool's own config
//! file is left in place.

use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use yaml_serde::Value;
use yamlpath::Route;

use crate::rules::{common, expression};
use crate::workflow::Workflow;
use crate::{filesystem, rules};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Tool {
    Zizmor,
    Actionlint,
    Poutine,
    Ghalint,
}

/// poutine's documented Action step is followed by this upload of the SARIF file it writes
/// (https://github.com/boostsecurityio/poutine/blob/2ad918cfe37a6889f05602264a03256eef149bdc/README.md#github-actions).
const UPLOAD_SARIF: &str = "github/codeql-action/upload-sarif@";

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Zizmor => "zizmor",
            Self::Actionlint => "actionlint",
            Self::Poutine => "poutine",
            Self::Ghalint => "ghalint",
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
            Self::Poutine => &["boostsecurityio/poutine-action@"],
            // ghalint documents no Action of its own, only the CLI.
            Self::Ghalint => &[],
        }
    }

    /// crosgha's Action accepts zizmor-action's inputs; none of actionlint's apply.
    fn keeps_inputs(self) -> bool {
        self == Self::Zizmor
    }
}

pub struct Summary {
    pub steps: usize,
    pub files: usize,
    pub config: Option<PathBuf>,
    /// What could not be migrated automatically, one line each.
    pub notes: Vec<String>,
}

type Edit = (Range<usize>, String);

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
        config: None,
        notes: Vec::new(),
    };
    let mut rewrites = Vec::new();
    let base = filesystem::base(path);
    for file in filesystem::discover(path)? {
        let located = |error: String| format!("{}: {error}", file.display());
        // A symbolic link is refused only when it needs an edit, below.
        let source = filesystem::read(&file)
            .map_err(|error| located(format!("cannot read workflow: {error}")))?;
        let workflow = Workflow::parse(source.text.clone()).map_err(located)?;
        let mut edits = Vec::new();
        let mut steps = 0;
        for job in common::jobs(&workflow) {
            // An install step goes only when a crosgha step replaced the tool in its job and no step
            // still runs it; the tool may also run indirectly (`pre-commit run`).
            let mut installs = Vec::new();
            let mut kept = false;
            let steps_before = steps;
            let job_steps = common::steps(&job);
            // poutine Action steps with the SARIF file each writes, and the SARIF uploads of the job.
            let mut poutine_steps = Vec::new();
            let mut uploads = Vec::new();
            for step in &job_steps {
                let place = || -> Result<String, String> {
                    let line = common::line(&workflow, &step.anchor())?;
                    Ok(format!("{}:{line}: {}", file.display(), step.location))
                };
                let action = step.uses().is_some_and(|uses| {
                    tool.actions().iter().any(|prefix| uses.starts_with(prefix))
                });
                // Replacing `uses` up to the end of its line would cut a flow mapping's `}`.
                if action && step_source(&workflow, step)?.starts_with('{') {
                    kept = true;
                    summary.notes.push(format!(
                        "{}: written as a flow mapping; replace this step with the crosgha Action \
                         by hand",
                        place()?
                    ));
                    continue;
                }
                if tool == Tool::Poutine
                    && let Some(uses) = step.uses()
                {
                    let with = |key: &str, default: &str| {
                        step.value
                            .get("with")
                            .and_then(|with| with.get(key))
                            .and_then(Value::as_str)
                            .unwrap_or(default)
                            .to_owned()
                    };
                    if uses.starts_with(UPLOAD_SARIF) {
                        uploads.push((step, with("sarif_file", "")));
                        continue;
                    }
                    if action {
                        // https://github.com/boostsecurityio/poutine-action/blob/badd750a259150be3b8b00823ad60849d539b96d/action.yml
                        let (format, output) =
                            (with("format", "sarif"), with("output", "results.sarif"));
                        if format == "sarif" {
                            poutine_steps.push((step, output));
                        } else {
                            kept = true;
                            summary.notes.push(format!(
                                "{}: writes poutine's `{format}` report to `{output}`, which the \
                                 crosgha Action does not produce; replace this step by hand",
                                place()?
                            ));
                        }
                        continue;
                    }
                }
                if action {
                    edits.push((
                        value_to_line_end(&workflow, &step.key("uses"))?,
                        replacement.clone(),
                    ));
                    if !tool.keeps_inputs() && step.value.get("with").is_some() {
                        edits.push((removal(&workflow, &step.key("with"))?, String::new()));
                    }
                    steps += 1;
                    continue;
                }
                let Some(script) = step.run() else { continue };
                let mut shell = classify(script, tool);
                // ghalint also reads its config path from the environment:
                // https://github.com/suzuki-shunsuke/ghalint/blob/82a9fe3de4e23d0b055812a12fbc724f0066b4bb/README.md#environment-variables
                if tool == Tool::Ghalint
                    && matches!(shell, Shell::Invoke(Ok(_)))
                    && step
                        .value
                        .get("env")
                        .and_then(|env| env.get("GHALINT_CONFIG"))
                        .is_some()
                {
                    shell = Shell::Invoke(Err(
                        "`GHALINT_CONFIG` has no crosgha Action input".to_owned()
                    ));
                }
                match shell {
                    Shell::Unrelated => {}
                    Shell::Install => {
                        installs.push((removal(&workflow, &step.route)?, place()?));
                    }
                    Shell::Invoke(Ok(inputs)) if step.value.get("working-directory").is_none() => {
                        edits.extend(shell_to_action(&workflow, step, &replacement, &inputs)?);
                        steps += 1;
                    }
                    Shell::Invoke(Err(reason)) => {
                        kept = true;
                        summary.notes.push(format!(
                            "{}: {reason}; replace this step with the crosgha Action by hand",
                            place()?
                        ));
                    }
                    Shell::Invoke(Ok(_)) => {
                        kept = true;
                        summary.notes.push(format!(
                            "{}: runs {} from a shell; replace it with the crosgha Action by hand",
                            place()?,
                            tool.name()
                        ));
                    }
                    Shell::Mixed => {
                        kept = true;
                        summary.notes.push(format!(
                            "{}: runs or installs {} next to other commands; the step was kept, \
                             migrate it by hand",
                            place()?,
                            tool.name()
                        ));
                    }
                }
            }
            // The crosgha Action uploads SARIF itself, so an upload of poutine's file goes; with no
            // such upload, poutine's report never reached the Security tab, and crosgha's does not
            // either. poutine fails only with `--fail-on-violation`, which its Action never passes.
            let mut removed_uploads = Vec::new();
            for (step, output) in poutine_steps {
                let uploaded: Vec<&common::Step> = uploads
                    .iter()
                    .filter(|(_, file)| *file == output)
                    .map(|(upload, _)| *upload)
                    .collect();
                // The crosgha Action keeps its SARIF in its own file, so a step that reads
                // poutine's would find nothing.
                let mut readers = Vec::new();
                for other in &job_steps {
                    let own = std::ptr::eq(other, step)
                        || uploaded.iter().any(|upload| std::ptr::eq(*upload, other));
                    if !own && step_source(&workflow, other)?.contains(output.as_str()) {
                        readers.push(other);
                    }
                }
                if let Some(reader) = readers.first() {
                    kept = true;
                    let line = common::line(&workflow, &reader.anchor())?;
                    summary.notes.push(format!(
                        "{}:{}: {}: reads `{output}`, which poutine wrote and the crosgha Action \
                         does not; the poutine step was kept, migrate it by hand",
                        file.display(),
                        line,
                        reader.location
                    ));
                    continue;
                }
                steps += 1;
                let mut inputs = vec![("fail-on", "none".to_owned())];
                if uploaded.is_empty() {
                    inputs.push(("advanced-security", "false".to_owned()));
                }
                let key = workflow
                    .document
                    .query_key_only(&step.key("uses"))
                    .map_err(|error| error.to_string())?;
                let indent = " ".repeat(key.location.point_span.0.1);
                edits.push((
                    value_to_line_end(&workflow, &step.key("uses"))?,
                    format!("{replacement}{}", with_block(&indent, &inputs)),
                ));
                if step.value.get("with").is_some() {
                    edits.push((removal(&workflow, &step.key("with"))?, String::new()));
                }
                for upload in uploaded {
                    let range = removal(&workflow, &upload.route)?;
                    if !removed_uploads.contains(&range) {
                        removed_uploads.push(range.clone());
                        edits.push((range, String::new()));
                    }
                }
            }
            if !kept && steps > steps_before {
                edits.extend(
                    installs
                        .into_iter()
                        .map(|(range, _)| (range, String::new())),
                );
            } else {
                for (_, place) in installs {
                    summary.notes.push(format!(
                        "{place}: installs {}, which a step that was not migrated may still use; \
                         the step was kept",
                        tool.name()
                    ));
                }
            }
        }
        if edits.is_empty() {
            continue;
        }
        filesystem::refuse_link(&file, &base).map_err(located)?;
        summary.steps += steps;
        summary.files += 1;
        let migrated = apply(workflow.document.source(), edits);
        Workflow::parse(migrated.clone())
            .map_err(|error| located(format!("migration produced invalid YAML: {error}")))?;
        rewrites.push((file, source, migrated));
    }

    let config = match filesystem::github_dir(path) {
        Some(github) => match tool {
            Tool::Zizmor => zizmor_config(&github, &mut summary.notes)?,
            Tool::Actionlint => actionlint_config(&github, &mut summary.notes)?,
            Tool::Poutine => {
                // https://github.com/boostsecurityio/poutine/blob/2ad918cfe37a6889f05602264a03256eef149bdc/README.md#configuration
                let root = github.parent().unwrap_or(&github);
                let configs = [root.join(".poutine.yml"), github.join("poutine.yml")];
                let keys = [
                    (
                        "skip",
                        "names poutine rules; ignore crosgha findings with `# crosgha: \
                         ignore[rule]` or `rules.<id>.ignore`",
                    ),
                    ("include", "adds Rego rules, which crosgha does not run"),
                ];
                config_notes(&configs, &keys, &mut summary.notes)?;
                None
            }
            Tool::Ghalint => {
                // `^(\.|\.github/)?ghalint\.ya?ml$`, per
                // https://github.com/suzuki-shunsuke/ghalint/blob/82a9fe3de4e23d0b055812a12fbc724f0066b4bb/README.md#configuration-file
                let root = github.parent().unwrap_or(&github);
                let configs = ["ghalint.yaml", "ghalint.yml"]
                    .into_iter()
                    .flat_map(|name| {
                        [
                            root.join(name),
                            root.join(format!(".{name}")),
                            github.join(name),
                        ]
                    })
                    .collect::<Vec<_>>();
                let keys = [(
                    "excludes",
                    "names ghalint policies; ignore crosgha findings with `# crosgha: \
                     ignore[rule]` or `rules.<id>.ignore`",
                )];
                config_notes(&configs, &keys, &mut summary.notes)?;
                None
            }
        },
        None => None,
    };

    // Every file is migrated in memory before any is written.
    for (file, source, migrated) in rewrites {
        filesystem::replace(&file, &base, &source, &migrated)
            .map_err(|error| format!("{}: {error}", file.display()))?;
    }
    if let Some((target, text)) = config {
        #[allow(
            clippy::disallowed_methods,
            reason = "writes the new crosgha.yml, not a workflow"
        )]
        fs::write(&target, text).map_err(|error| format!("{}: {error}", target.display()))?;
        summary.config = Some(target);
    }
    Ok(summary)
}

fn apply(source: &str, mut edits: Vec<Edit>) -> String {
    let mut output = source.to_owned();
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, text) in edits {
        output.replace_range(range, &text);
    }
    output
}

/// The lines of the node at `route`. The parser counts comment lines after a block's last node
/// as part of it, though they belong to what follows (a comment over the next job), so they stay.
fn removal(workflow: &Workflow, route: &Route) -> Result<Range<usize>, String> {
    let span = workflow
        .document
        .removal_span(route)
        .map_err(|error| error.to_string())?;
    let source = workflow.document.source();
    let mut lines: Vec<&str> = source[span.clone()].split_inclusive('\n').collect();
    while lines.len() > 1
        && lines.last().is_some_and(|line| {
            let line = line.trim();
            line.is_empty() || line.starts_with('#')
        })
    {
        lines.pop();
    }
    Ok(span.start..span.start + lines.concat().len())
}

fn step_source<'w>(workflow: &'w Workflow, step: &common::Step) -> Result<&'w str, String> {
    let feature = workflow
        .document
        .query_pretty(&step.route)
        .map_err(|error| error.to_string())?;
    Ok(workflow.document.extract(&feature))
}

/// The `uses` value through the end of its line, so the old `# vX.Y.Z` comment goes with it.
fn value_to_line_end(workflow: &Workflow, route: &Route) -> Result<Range<usize>, String> {
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

enum Shell {
    Unrelated,
    /// Every line installs the tool.
    Install,
    /// One line runs the tool, the others install it; the translated Action inputs.
    Invoke(Result<Vec<(&'static str, String)>, String>),
    /// The tool runs or is installed next to other commands.
    Mixed,
}

fn classify(script: &str, tool: Tool) -> Shell {
    let lines: Vec<&str> = script
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    let related =
        |line: &&str| mentions(line, tool.name()) || invocation_args(&words(line), tool).is_some();
    if !lines.iter().any(related) {
        return Shell::Unrelated;
    }
    let installs = |line: &&str| installs(line, tool);
    let invocations: Vec<Vec<String>> = lines
        .iter()
        .filter(|line| !installs(line))
        .map(|line| words(line))
        .collect();
    match invocations.as_slice() {
        [] => Shell::Install,
        [words] => match invocation_args(words, tool) {
            Some(args) => Shell::Invoke(translate(args, tool)),
            None => Shell::Mixed,
        },
        _ => Shell::Mixed,
    }
}

fn mentions(text: &str, tool: &str) -> bool {
    text.split(|character: char| {
        !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
    })
    .any(|word| word == tool)
}

/// Whether the line is one command that installs only the tool. A step made of such lines is
/// removed, so anything else it could run (`pip install zizmor ruff`, `... && npm ci`) must fail.
fn installs(line: &str, tool: Tool) -> bool {
    if ["&&", "||", ";"].iter().any(|chain| line.contains(chain)) {
        return false;
    }
    // actionlint's documented installer: `bash <(curl .../download-actionlint.bash)`.
    if tool == Tool::Actionlint && line.contains("download-actionlint.bash") {
        return true;
    }
    if line.contains(['|', '`']) || line.contains("$(") {
        return false;
    }
    let words = words(line);
    let words: Vec<&str> = words
        .iter()
        .map(String::as_str)
        .skip_while(|word| *word == "sudo")
        .collect();
    let Some(install) = words.iter().position(|word| *word == "install") else {
        return false;
    };
    let manager = match tool {
        Tool::Zizmor => matches!(
            words[..install],
            ["pip" | "pip3" | "pipx" | "cargo" | "brew"]
                | ["uv", "tool" | "pip"]
                | ["python" | "python3", "-m", "pip"]
        ),
        Tool::Actionlint => matches!(words[..install], ["go" | "brew"]),
        Tool::Poutine => matches!(words[..install], ["brew"]),
        Tool::Ghalint => matches!(words[..install], ["go" | "brew"]),
    };
    let mut packages = words[install + 1..]
        .iter()
        .filter(|word| !word.starts_with('-'))
        .peekable();
    manager
        && packages.peek().is_some()
        && packages.all(|package| match tool {
            Tool::Zizmor => package.strip_prefix("zizmor").is_some_and(|version| {
                version.is_empty() || version.starts_with(['=', '<', '>', '!', '~', '@'])
            }),
            Tool::Actionlint => {
                *package == "actionlint" || package.starts_with("github.com/rhysd/actionlint/")
            }
            Tool::Poutine => *package == "poutine",
            // https://github.com/suzuki-shunsuke/ghalint/blob/82a9fe3de4e23d0b055812a12fbc724f0066b4bb/docs/install.md
            Tool::Ghalint => {
                ["ghalint", "suzuki-shunsuke/ghalint/ghalint"].contains(package)
                    || package.starts_with("github.com/suzuki-shunsuke/ghalint/cmd/ghalint@")
            }
        })
}

/// Shell words, keeping quoted text and `${{ ... }}` expressions whole.
fn words(line: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut expression = false;
    for character in line.chars() {
        match (quote, character) {
            (Some(open), _) if character == open => quote = None,
            (Some(_), _) => current.push(character),
            (None, '\'' | '"') if !expression => quote = Some(character),
            (None, ' ' | '\t') if !expression => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => {
                current.push(character);
                if current.ends_with("${{") {
                    expression = true;
                } else if expression && current.ends_with("}}") {
                    expression = false;
                }
            }
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// The arguments after the tool's command, or `None` when the line runs something else.
fn invocation_args(words: &[String], tool: Tool) -> Option<&[String]> {
    let names: Vec<&str> = words.iter().map(String::as_str).collect();
    let is_zizmor = |word: &str| {
        word == "zizmor" || word.starts_with("zizmor@") || word.starts_with("zizmor==")
    };
    let skip = match (tool, names.as_slice()) {
        (Tool::Zizmor, [first, ..]) if is_zizmor(first) => 1,
        (Tool::Zizmor, ["uvx", second, ..]) if is_zizmor(second) => 2,
        (Tool::Zizmor, ["pipx", "run", third, ..]) if is_zizmor(third) => 3,
        (Tool::Zizmor, ["uv", "tool", "run", fourth, ..]) if is_zizmor(fourth) => 4,
        (Tool::Zizmor, ["python" | "python3", "-m", "zizmor", ..]) => 3,
        (Tool::Actionlint, [first, ..])
            if *first == "actionlint"
                || first.ends_with("/actionlint")
                || (first.starts_with("${{") && first.contains("outputs.executable")) =>
        {
            1
        }
        (Tool::Poutine, [first, ..]) if *first == "poutine" || first.ends_with("/poutine") => 1,
        (Tool::Ghalint, [first, ..]) if *first == "ghalint" || first.ends_with("/ghalint") => 1,
        _ => return None,
    };
    Some(&words[skip..])
}

/// Action inputs for the tool's CLI arguments, in the order they are written. A CLI run never
/// uploaded SARIF, so `advanced-security` is turned off to keep behavior the same.
fn translate(args: &[String], tool: Tool) -> Result<Vec<(&'static str, String)>, String> {
    // Only `analyze_local` scans the checkout, as the crosgha Action does; the other commands read
    // repositories through an SCM API.
    let args = match (tool, args.split_first()) {
        (Tool::Poutine, Some((command, rest))) if command == "analyze_local" => rest,
        (Tool::Poutine, Some((command, _))) => {
            return Err(format!(
                "`poutine {command}` has no crosgha Action equivalent; the Action scans the \
                 checkout, as `poutine analyze_local` does"
            ));
        }
        (Tool::Poutine, None) => return Err("`poutine` runs no command".to_owned()),
        // `run` checks the workflows and `run-action` (alias `act`) the action files it names:
        // https://github.com/suzuki-shunsuke/ghalint/blob/82a9fe3de4e23d0b055812a12fbc724f0066b4bb/README.md#how-to-use
        (Tool::Ghalint, Some((command, rest))) => match command.as_str() {
            "run" => rest,
            "run-action" | "act" if !rest.is_empty() => rest,
            "run-action" | "act" => {
                return Err(format!(
                    "`ghalint {command}` without files checks every action file up to three \
                     directories deep; list them in `inputs` by hand"
                ));
            }
            flag if flag.starts_with('-') => {
                return Err(format!("`{flag}` has no crosgha Action input"));
            }
            _ => {
                return Err(format!(
                    "`ghalint {command}` has no crosgha Action equivalent"
                ));
            }
        },
        (Tool::Ghalint, None) => return Err("`ghalint` runs no command".to_owned()),
        _ => args,
    };
    let mut fail_on_violation = false;
    let mut inputs: Vec<String> = Vec::new();
    let mut settings: Vec<(&'static str, String)> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        index += 1;
        if shell_syntax(arg) {
            return Err(format!(
                "`{arg}` uses shell syntax (a chain, pipe, redirect, or expansion) that Action \
                 inputs cannot express"
            ));
        }
        if !arg.starts_with('-') {
            inputs.push(arg.to_owned());
            continue;
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) => (flag, Some(value.to_owned())),
            None => (arg, None),
        };
        let mut value = || {
            inline.clone().or_else(|| {
                index += 1;
                args.get(index - 1).cloned()
            })
        };
        let setting = match (tool, flag) {
            (Tool::Zizmor, "--persona") => Some(("persona", value())),
            (Tool::Zizmor, "--pedantic") => Some(("persona", Some("pedantic".to_owned()))),
            (Tool::Zizmor, "--min-severity") => Some(("min-severity", value())),
            (Tool::Zizmor, "--min-confidence") => Some(("min-confidence", value())),
            (Tool::Zizmor, "--offline" | "--no-online-audits") => {
                Some(("online-audits", Some("false".to_owned())))
            }
            (Tool::Zizmor, "--config") => Some(("config", value())),
            (Tool::Zizmor, "--gh-token") => Some(("token", value())),
            // The Action prints text (zizmor's `plain`) or annotations, and keeps SARIF in a file;
            // a step whose log carries JSON or SARIF would lose it.
            (Tool::Zizmor | Tool::Poutine, "--format") => match value() {
                Some(format) if format == "plain" && tool == Tool::Zizmor => None,
                Some(format) if format == "pretty" && tool == Tool::Poutine => None,
                Some(format) if format == "github" && tool == Tool::Zizmor => {
                    Some(("annotations", Some("true".to_owned())))
                }
                Some(format) => {
                    return Err(format!(
                        "`--format {format}` has no crosgha Action input; the Action prints text \
                         and writes SARIF to its `output-file` output"
                    ));
                }
                None => None,
            },
            (Tool::Zizmor, "--color" | "--cache-dir") => {
                if inline.is_none() && flag == "--cache-dir" {
                    value();
                }
                None
            }
            (Tool::Zizmor, "-q" | "--quiet" | "-v" | "--verbose" | "--no-progress") => None,
            (Tool::Actionlint, "-color" | "-no-color" | "-verbose" | "-debug" | "-oneline") => None,
            (Tool::Actionlint, "-format") => {
                return Err(
                    "`-format` has no crosgha Action input; the Action prints text and writes \
                     SARIF to its `output-file` output"
                        .to_owned(),
                );
            }
            (Tool::Actionlint, "-shellcheck" | "-pyflakes") => {
                value();
                None
            }
            // https://github.com/boostsecurityio/poutine/blob/2ad918cfe37a6889f05602264a03256eef149bdc/README.md#configuration-options
            (Tool::Poutine, "--token") => Some(("token", value())),
            (Tool::Poutine, "--fail-on-violation") => {
                fail_on_violation = true;
                None
            }
            (Tool::Poutine, "--verbose" | "--disable-version-check" | "--ignore-forks") => None,
            (Tool::Poutine, "--threads" | "--scm-base-url") => {
                if inline.is_none() {
                    value();
                }
                None
            }
            (Tool::Poutine, "--scm") => match value() {
                Some(scm) if scm == "github" => None,
                scm => {
                    return Err(format!(
                        "`--scm {}` has no crosgha Action input; crosgha checks GitHub Actions only",
                        scm.unwrap_or_default()
                    ));
                }
            },
            _ => return Err(format!("`{flag}` has no crosgha Action input")),
        };
        if let Some((key, value)) = setting {
            let value = value.ok_or_else(|| format!("`{flag}` has no value"))?;
            settings.retain(|(existing, _)| *existing != key);
            settings.push((key, value));
        }
    }
    if tool == Tool::Poutine && !fail_on_violation {
        settings.push(("fail-on", "none".to_owned()));
    }
    let mut ordered = Vec::new();
    if !inputs.is_empty() && inputs != ["."] {
        ordered.push(("inputs", inputs.join(" ")));
    }
    for key in [
        "persona",
        "min-severity",
        "min-confidence",
        "online-audits",
        "config",
        "token",
        "fail-on",
    ] {
        if let Some((_, value)) = settings.iter().find(|(existing, _)| *existing == key) {
            ordered.push((key, value.clone()));
        }
    }
    ordered.push(("advanced-security", "false".to_owned()));
    if let Some((_, value)) = settings.iter().find(|(key, _)| *key == "annotations") {
        ordered.push(("annotations", value.clone()));
    }
    Ok(ordered)
}

/// Whether the word holds shell syntax anywhere (`.;`, `2>/dev/null`, `$(...)`, `$VAR`); an
/// Action input would pass it on literally. `${{ ... }}` is expanded by Actions, not the shell.
fn shell_syntax(word: &str) -> bool {
    let expressions = expression::spans(word);
    word.char_indices().any(|(index, char)| {
        ";><|&`$".contains(char)
            && !expressions
                .iter()
                .any(|&(start, end)| (start..end).contains(&index))
    })
}

/// Replaces the step's `run:` with `uses:` and `with:`, and drops `shell:`, which `uses`
/// steps cannot have.
fn shell_to_action(
    workflow: &Workflow,
    step: &common::Step,
    replacement: &str,
    inputs: &[(&'static str, String)],
) -> Result<Vec<Edit>, String> {
    let run = workflow
        .document
        .query_pretty(&step.key("run"))
        .map_err(|error| error.to_string())?;
    let (start, end) = run.location.byte_span;
    let indent = " ".repeat(run.location.point_span.0.1);
    let mut text = format!("uses: {replacement}{}", with_block(&indent, inputs));
    let source = workflow.document.source();
    if source[start..end].ends_with('\n') {
        text.push('\n');
    }
    let mut edits = vec![(start..end, text)];
    if step.value.get("shell").is_some() {
        edits.push((removal(workflow, &step.key("shell"))?, String::new()));
    }
    Ok(edits)
}

/// A `with:` block of `inputs` for a step whose keys start at `indent`, on the lines after its
/// `uses:`.
fn with_block(indent: &str, inputs: &[(&'static str, String)]) -> String {
    use std::fmt::Write as _;

    let mut text = String::new();
    if !inputs.is_empty() {
        let _ = write!(text, "\n{indent}with:");
        for (key, value) in inputs {
            let _ = write!(text, "\n{indent}  {key}: {}", scalar(value));
        }
    }
    text
}

/// A YAML scalar for an Action input: booleans are quoted strings, as Action inputs are.
fn scalar(value: &str) -> String {
    let plain = value.starts_with("${{")
        || value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._/-*".contains(character));
    if matches!(value, "true" | "false") || !plain || value.is_empty() {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        value.to_owned()
    }
}

/// `crosgha.yml` next to `zizmor.yml`: rule keys that name exactly one crosgha rule are renamed,
/// keys crosgha has no rule for are dropped, and comments stay.
fn zizmor_config(
    github: &Path,
    notes: &mut Vec<String>,
) -> Result<Option<(PathBuf, String)>, String> {
    let root = github.parent().unwrap_or(github);
    let Some(source_path) = [
        github.join("zizmor.yml"),
        github.join("zizmor.yaml"),
        root.join("zizmor.yml"),
        root.join("zizmor.yaml"),
    ]
    .into_iter()
    .find(|path| path.is_file()) else {
        return Ok(None);
    };
    let target = source_path.with_file_name("crosgha.yml");
    if target.exists() {
        notes.push(format!(
            "{} exists; the config was not migrated",
            target.display()
        ));
        return Ok(None);
    }
    let located = |error: String| format!("{}: {error}", source_path.display());
    let source = fs::read_to_string(&source_path).map_err(|error| located(error.to_string()))?;
    let config = Workflow::parse_yaml(source).map_err(located)?;
    let mut edits = Vec::new();
    let ids: Vec<String> = config
        .value
        .get("rules")
        .and_then(Value::as_mapping)
        .into_iter()
        .flatten()
        .filter_map(|(id, _)| id.as_str().map(str::to_owned))
        .collect();
    for id in &ids {
        let id = id.clone();
        let route = Route::default().with_key("rules").with_key(id.clone());
        let Some((rule, _)) = rules::all().find(|(rule, _)| rule.answers_to(&id)) else {
            notes.push(format!(
                "`{id}` has no crosgha rule; it was dropped from crosgha.yml"
            ));
            edits.push((removal(&config, &route)?, String::new()));
            continue;
        };
        if rule.id == id {
            continue;
        }
        let zizmor_ids = rule
            .equivalents
            .iter()
            .filter(|(tool, _)| *tool == "zizmor")
            .count();
        // A key for part of a rule (`unpinned-images`) keeps its zizmor name, which crosgha
        // applies to that part only.
        if zizmor_ids == 1 && ids.iter().any(|other| other == rule.id) {
            // A second key with the same name would make the config invalid YAML for crosgha.
            notes.push(format!(
                "`{id}` was not renamed: `{}` is already set; merge the two by hand",
                rule.id
            ));
        } else if zizmor_ids == 1 {
            let key = config
                .document
                .query_key_only(&route)
                .map_err(|error| error.to_string())?;
            let (start, end) = key.location.byte_span;
            edits.push((start..end, rule.id.to_owned()));
        }
    }
    Ok(Some((target, apply(config.document.source(), edits))))
}

/// `crosgha.yml` from `actionlint.yaml`: self-hosted runner labels carry over; the rest has no
/// crosgha meaning and is reported.
fn actionlint_config(
    github: &Path,
    notes: &mut Vec<String>,
) -> Result<Option<(PathBuf, String)>, String> {
    use std::fmt::Write as _;

    let Some((name, source_path)) = ["actionlint.yaml", "actionlint.yml"]
        .into_iter()
        .map(|name| (name, github.join(name)))
        .find(|(_, path)| path.is_file())
    else {
        return Ok(None);
    };
    let located = |error: String| format!("{}: {error}", source_path.display());
    let source = fs::read_to_string(&source_path).map_err(|error| located(error.to_string()))?;
    let config: Value =
        yaml_serde::from_str(&source).map_err(|error| located(error.to_string()))?;
    if config.get("config-variables").is_some() {
        notes.push(format!(
            ".github/{name}: `config-variables` has no crosgha equivalent"
        ));
    }
    if config.get("paths").is_some() {
        notes.push(format!(
            ".github/{name}: `paths` ignores match actionlint messages; use `# crosgha: ignore[rule]` or `rules.<id>.ignore`"
        ));
    }
    let labels: Vec<&str> = config
        .get("self-hosted-runner")
        .and_then(|runner| runner.get("labels"))
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if labels.is_empty() {
        return Ok(None);
    }
    let target = github.join("crosgha.yml");
    if target.exists() {
        notes.push(format!(
            "{} exists; the config was not migrated",
            target.display()
        ));
        return Ok(None);
    }
    let mut text = format!(
        "# Migrated from .github/{name} by `crosgha migrate --from actionlint`.\nrules:\n  self-hosted-runner:\n    config:\n      labels:\n"
    );
    for label in labels {
        // A JSON string is a valid YAML double-quoted scalar.
        let _ = writeln!(text, "        - {}", serde_json::Value::from(label));
    }
    Ok(Some((target, text)))
}

/// Notes for the `keys` of the first file in `configs` (in precedence order) that exists. The
/// configs of poutine and ghalint name the tool's own rules or extensions, which crosgha cannot
/// read, so no `crosgha.yml` is written from them.
fn config_notes(
    configs: &[PathBuf],
    keys: &[(&str, &str)],
    notes: &mut Vec<String>,
) -> Result<(), String> {
    let Some(source_path) = configs.iter().find(|path| path.is_file()) else {
        return Ok(());
    };
    let located = |error: String| format!("{}: {error}", source_path.display());
    let source = fs::read_to_string(source_path).map_err(|error| located(error.to_string()))?;
    let config: Value =
        yaml_serde::from_str(&source).map_err(|error| located(error.to_string()))?;
    for (key, reason) in keys {
        if config.get(key).is_some() {
            notes.push(located(format!("`{key}` {reason}")));
        }
    }
    Ok(())
}
