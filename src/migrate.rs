//! `crosgha migrate --from <tool>`: moves a repository from zizmor or actionlint to crosgha.
//! It rewrites the tool's Action steps, turns shell steps that only install and run the tool into
//! crosgha Action steps, and writes a `crosgha.yml` from the tool's config. The tool's own config
//! file is left in place.

use std::{
    fs,
    ops::Range,
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use yaml_serde::Value;
use yamlpath::Route;

use crate::{filesystem, rules, rules::common, workflow::Workflow};

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
    for file in filesystem::discover(path)? {
        let located = |error: String| format!("{}: {error}", file.display());
        let source = filesystem::read(&file, true).map_err(located)?;
        let workflow = Workflow::parse(source).map_err(located)?;
        let mut edits = Vec::new();
        let mut steps = 0;
        for job in common::jobs(&workflow) {
            // An install step goes only when a crosgha step replaced the tool in its job and no step
            // still runs it; the tool may also run indirectly (`pre-commit run`).
            let mut installs = Vec::new();
            let mut kept = false;
            let steps_before = steps;
            for step in common::steps(&job) {
                let place = || -> Result<String, String> {
                    let line = common::line(&workflow, &step.anchor())?;
                    Ok(format!("{}:{line}: {}", file.display(), step.location))
                };
                if step.uses().is_some_and(|uses| {
                    tool.actions().iter().any(|prefix| uses.starts_with(prefix))
                }) {
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
                match classify(script, tool) {
                    Shell::Unrelated => {}
                    Shell::Install => {
                        installs.push((removal(&workflow, &step.route)?, place()?));
                    }
                    Shell::Invoke(Ok(inputs)) if step.value.get("working-directory").is_none() => {
                        edits.extend(shell_to_action(&workflow, &step, &replacement, &inputs)?);
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
        summary.steps += steps;
        summary.files += 1;
        let migrated = apply(workflow.document.source(), edits);
        Workflow::parse(migrated.clone())
            .map_err(|error| located(format!("migration produced invalid YAML: {error}")))?;
        rewrites.push((file, migrated));
    }

    let config = match filesystem::github_dir(path) {
        Some(github) => match tool {
            Tool::Zizmor => zizmor_config(&github, &mut summary.notes)?,
            Tool::Actionlint => actionlint_config(&github, &mut summary.notes)?,
        },
        None => None,
    };

    // Every file is migrated in memory before any is written.
    for (file, migrated) in rewrites {
        filesystem::write_verified(&file, &migrated)
            .map_err(|error| format!("{}: {error}", file.display()))?;
    }
    if let Some((target, text)) = config {
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

fn removal(workflow: &Workflow, route: &Route) -> Result<Range<usize>, String> {
    workflow
        .document
        .removal_span(route)
        .map_err(|error| error.to_string())
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
        _ => return None,
    };
    Some(&words[skip..])
}

/// Action inputs for the tool's CLI arguments, in the order they are written. A CLI run never
/// uploaded SARIF, so `advanced-security` is turned off to keep behavior the same.
fn translate(args: &[String], tool: Tool) -> Result<Vec<(&'static str, String)>, String> {
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
            (Tool::Zizmor, "--format") => match value() {
                Some(format) if format == "plain" => None,
                Some(format) if format == "github" => {
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
            _ => return Err(format!("`{flag}` has no crosgha Action input")),
        };
        if let Some((key, value)) = setting {
            let value = value.ok_or_else(|| format!("`{flag}` has no value"))?;
            settings.retain(|(existing, _)| *existing != key);
            settings.push((key, value));
        }
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
    let mut rest = word;
    let mut outside = String::new();
    while let Some(start) = rest.find("${{") {
        outside.push_str(&rest[..start]);
        match rest[start..].find("}}") {
            Some(end) => rest = &rest[start + end + 2..],
            None => {
                rest = &rest[start..];
                break;
            }
        }
    }
    outside.push_str(rest);
    outside.contains([';', '>', '<', '|', '&', '`', '$'])
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
    let mut text = format!("uses: {replacement}");
    if !inputs.is_empty() {
        text.push_str(&format!("\n{indent}with:"));
        for (key, value) in inputs {
            text.push_str(&format!("\n{indent}  {key}: {}", scalar(value)));
        }
    }
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
    for id in ids {
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
        if zizmor_ids == 1 {
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
        text.push_str(&format!("        - {}\n", serde_json::Value::from(label)));
    }
    Ok(Some((target, text)))
}
