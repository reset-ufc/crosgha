//! GITHUB_ENV and GITHUB_PATH writes: the shell, PowerShell, cmd, and Python forms that write an
//! environment file, and whether the written text comes from an outsider.

use yaml_serde::Value;

use super::{
    Finding, Rule,
    common::{self, PRIVILEGED_TRIGGERS},
    expression,
};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// zizmor `github-env` and CodeQL `actions/envvar-injection`: environment files written in
// privileged runs, or with untrusted values, change later steps.
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#setting-an-environment-variable
pub const RULE: Rule = Rule {
    id: "github-env-injection",
    summary: "GITHUB_ENV and GITHUB_PATH must not be written from untrusted data or in privileged runs.",
    tag: "github-env-injection",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "github-env")],
    configurable: false,
    relational: false,
};

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let privileged = common::has_trigger(workflow, PRIVILEGED_TRIGGERS);
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some(script) = step.run() else { continue };
            let tainted = tainted_variables(workflow, &job, &step);
            let unsafe_write = env_file_writes(script).iter().any(|written| {
                privileged
                    || !expression::untrusted_in_text(written).is_empty()
                    || tainted.iter().any(|name| reads_variable(written, name))
            });
            if unsafe_write {
                findings.push(common::finding(
                    workflow,
                    &RULE,
                    step.key("run"),
                    step.location.clone(),
                    "writes GITHUB_ENV or GITHUB_PATH from an untrusted context",
                )?);
            }
        }
    }
    Ok(findings)
}

/// Lowercase names of the environment variables a step sees with an untrusted value. Step `env`
/// overrides the job's, which overrides the workflow's.
fn tainted_variables(workflow: &Workflow, job: &common::Job, step: &common::Step) -> Vec<String> {
    let mut untrusted = std::collections::BTreeMap::new();
    for scope in [&workflow.value, job.value, step.value] {
        let Some(env) = scope.get("env").and_then(Value::as_mapping) else {
            continue;
        };
        for (name, value) in env {
            if let Some(name) = name.as_str() {
                let tainted = value
                    .as_str()
                    .is_some_and(|text| !expression::untrusted_in_text(text).is_empty());
                untrusted.insert(name.to_ascii_lowercase(), tainted);
            }
        }
    }
    untrusted
        .into_iter()
        .filter_map(|(name, tainted)| tainted.then_some(name))
        .collect()
}

/// The text of every command in `script` that writes GITHUB_ENV or GITHUB_PATH: the line with
/// the write, plus the heredoc it reads, the `{ }`, `( )`, or loop body it closes, or the
/// indented block of a Python `with open(...):` it opens.
fn env_file_writes(script: &str) -> Vec<String> {
    let script = script.replace("\\\n", " ");
    let lines: Vec<&str> = script.lines().collect();
    (0..lines.len())
        .filter(|&index| writes_env_file(lines[index]))
        .map(|index| {
            let end = heredoc_end(&lines, index).max(indented_block_end(&lines, index));
            lines[block_start(&lines, index)..=end].join("\n")
        })
        .collect()
}

const ENV_FILES: &[&str] = &["github_env", "github_path"];
const FILE_WRITERS: &[&str] = &["tee", "out-file", "add-content", "set-content"];

/// Whether `line` writes an environment file: a `>`, `>|`, or `>>` redirect into it, `tee` or
/// PowerShell's `Out-File`, `Add-Content`, or `Set-Content` naming it, or Python opening it to
/// append or write. Reading the path is not a write. Compared in lower case because PowerShell
/// and cmd ignore it.
fn writes_env_file(line: &str) -> bool {
    let line = env_file_expressions(&line.to_ascii_lowercase());
    ENV_FILES.iter().any(|file| {
        line.match_indices(file).any(|(at, _)| {
            let whole_name = !line[at + file.len()..]
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
            if whole_name && python_opens_for_writing(&line[..at], &line[at + file.len()..]) {
                return true;
            }
            let Some(before) = ["${", "$env:", "$", "%"]
                .iter()
                .find_map(|sigil| line[..at].strip_suffix(sigil))
            else {
                return false;
            };
            let before = before.trim_end_matches([' ', '\t', '"', '\'']);
            let command = before.rsplit(['|', ';', '&']).next().unwrap_or(before);
            let writer = !before.ends_with('<')
                && command
                    .split_whitespace()
                    .any(|word| FILE_WRITERS.contains(&word));
            whole_name && (before.ends_with('>') || before.ends_with(">|") || writer)
        })
    })
}

/// Rewrites `${{ github.env }}` and `${{ github.path }}`, the contexts that hold the same paths
/// (https://docs.github.com/en/actions/reference/workflows-and-actions/contexts#github-context),
/// to `$github_env` and `$github_path`.
fn env_file_expressions(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut copied = 0;
    for (start, end) in expression::spans(line) {
        out.push_str(&line[copied..start]);
        out.push_str(match line[start + 3..end - 2].trim() {
            "github.env" => "$github_env",
            "github.path" => "$github_path",
            _ => &line[start..end],
        });
        copied = end;
    }
    out.push_str(&line[copied..]);
    out
}

/// Whether the environment file named between `before` and `after` is Python's
/// `os.environ["…"]`, `os.environ.get("…")`, or `os.getenv("…")` opened to append or write:
/// `open(…, "a")`, `open(…, mode="w")`, `Path(…).open("a")`, or `Path(…).write_text(…)`.
fn python_opens_for_writing(before: &str, after: &str) -> bool {
    let Some(quote) = before.chars().last().filter(|char| "'\"".contains(*char)) else {
        return false;
    };
    let lookup = before[..before.len() - 1].trim_end();
    let from_environment = ["environ[", "environ.get(", "getenv("]
        .iter()
        .any(|call| lookup.ends_with(call));
    let Some(after) = after.strip_prefix(quote) else {
        return false;
    };
    let after = after.trim_start_matches([']', ')', ' ']);
    let writing_mode = |text: &str| {
        let text = text.trim_start();
        let text = text.strip_prefix("mode=").unwrap_or(text).trim_start();
        text.strip_prefix(['\'', '"'])
            .is_some_and(|mode| mode.starts_with(['a', 'w']))
    };
    from_environment
        && (after.strip_prefix(',').is_some_and(writing_mode)
            || after.strip_prefix(".open(").is_some_and(writing_mode)
            || after.starts_with(".write_text("))
}

/// Whether `text` expands the environment variable `name` (lowercase) in shell (`$name`,
/// `${name}`), PowerShell (`$env:name`), cmd (`%name%`), or Python (`os.environ["name"]`).
fn reads_variable(text: &str, name: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.match_indices(name).any(|(at, _)| {
        let before = &text[..at];
        let after = &text[at + name.len()..];
        let ends_name = !after
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        let quoted = ['\'', '"']
            .into_iter()
            .any(|quote| before.ends_with(quote) && after.starts_with(quote));
        let sigil = before.ends_with("$env:")
            || before.ends_with("${")
            || (before.ends_with('%') && after.starts_with('%'))
            || before.ends_with('$');
        ends_name && (sigil || quoted)
    })
}

/// Last line of the indented block that `lines[index]` opens with a trailing `:`, as Python's
/// `with open(...) as file:` does, or `index` without one.
fn indented_block_end(lines: &[&str], index: usize) -> usize {
    let opener = lines[index];
    if !opener.trim_end().ends_with(':') {
        return index;
    }
    let indent = |line: &str| line.len() - line.trim_start().len();
    let depth = indent(opener);
    (index + 1..lines.len())
        .take_while(|&next| lines[next].trim().is_empty() || indent(lines[next]) > depth)
        .last()
        .unwrap_or(index)
}

/// First line of the block that the write on `lines[index]` redirects: the matching `{`, `(`,
/// or `do` line when it starts with `}`, `)`, or `done`.
// ponytail: the nearest opener indented no deeper than the closer; an unindented nested block
// of the same kind cuts the body short. Parse the shell when that shows up in practice.
fn block_start(lines: &[&str], index: usize) -> usize {
    let closer = lines[index].trim_start();
    let indent = lines[index].len() - closer.len();
    let opens: fn(&str) -> bool = if closer.starts_with('}') {
        |line| line.ends_with('{')
    } else if closer.starts_with(')') {
        |line| line.starts_with('(')
    } else if closer.starts_with("done") {
        |line| line.ends_with("do")
    } else {
        return index;
    };
    (0..index)
        .rev()
        .find(|&opener| {
            let text = lines[opener].trim_start();
            lines[opener].len() - text.len() <= indent && opens(text.trim_end())
        })
        .unwrap_or(0)
}

/// Last line of the heredoc that the write on `lines[index]` reads, or `index` without one.
fn heredoc_end(lines: &[&str], index: usize) -> usize {
    let line = lines[index];
    let Some(at) = line
        .find("<<")
        .filter(|&at| !line[at + 2..].starts_with('<'))
    else {
        return index;
    };
    let delimiter: String = line[at + 2..]
        .trim_start_matches('-')
        .trim_start()
        .chars()
        .take_while(|char| !char.is_whitespace() && !";|&)".contains(*char))
        .filter(|char| !"'\"".contains(*char))
        .collect();
    if delimiter.is_empty() {
        return index;
    }
    (index + 1..lines.len())
        .find(|&end| lines[end].trim() == delimiter)
        .unwrap_or(lines.len() - 1)
}
