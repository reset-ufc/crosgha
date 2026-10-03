use std::collections::BTreeSet;

use yaml_serde::Value;
use yamlpath::Route;

use super::{
    Finding, Rule,
    common::{self, EXTERNAL_TRIGGERS, PRIVILEGED_TRIGGERS},
    maintainability,
};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// GitHub: untrusted context values expanded into scripts are a script injection risk.
// https://docs.github.com/en/actions/concepts/security/script-injections
pub const SCRIPT_INJECTION: Rule = Rule {
    id: "script-injection",
    summary: "Untrusted event data must not be expanded into scripts with `${{ }}`.",
    tag: "script-injection",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[
        ("zizmor", "template-injection"),
        ("actionlint", "expression"),
    ],
    configurable: false,
};

// zizmor `github-env` and CodeQL `actions/envvar-injection`: environment files written in
// privileged runs, or with untrusted values, change later steps.
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#setting-an-environment-variable
pub const ENV_INJECTION: Rule = Rule {
    id: "github-env-injection",
    summary: "GITHUB_ENV and GITHUB_PATH must not be written from untrusted data or in privileged runs.",
    tag: "github-env-injection",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "github-env")],
    configurable: false,
};

// TaintAWI (arXiv 2605.07135) Prompt-to-Agent: untrusted event data reaches an agent prompt.
pub const PROMPT_INJECTION: Rule = Rule {
    id: "llm-prompt-injection",
    summary: "AI agent actions must not receive untrusted event data or run on events outsiders trigger.",
    tag: "llm-prompt-injection",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[],
    configurable: false,
};

// GitHub Security Lab "pwn request": privileged triggers that check out untrusted code.
// https://securitylab.github.com/resources/github-actions-preventing-pwn-requests/
pub const PWN_REQUEST: Rule = Rule {
    id: "pwn-request",
    summary: "Privileged triggers must not check out or run pull request code.",
    tag: "pwn-request",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[],
    configurable: false,
};

// zizmor `dangerous-triggers`: triggers that run with base-repository privileges.
pub const DANGEROUS_TRIGGER: Rule = Rule {
    id: "dangerous-trigger",
    summary: "pull_request_target and workflow_run run with base repository privileges.",
    tag: "dangerous-trigger",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    equivalents: &[("zizmor", "dangerous-triggers")],
    configurable: false,
};

// actionlint `if-cond` constant conditions, zizmor `unsound-condition` and `bot-conditions`.
// GitHub evaluates `if` as an expression only when `${{ }}` wraps the whole value:
// https://docs.github.com/en/actions/reference/workflows-and-actions/expressions
pub const UNSAFE_IF: Rule = Rule {
    id: "unsafe-if-condition",
    summary: "Conditions must not be constant, always true, or based on spoofable bot names.",
    tag: "unsafe-if-condition",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[
        ("zizmor", "bot-conditions"),
        ("zizmor", "unsound-condition"),
        ("actionlint", "if-cond"),
    ],
    configurable: false,
};

// CodeQL `actions/artifact-poisoning`: artifacts from another run extracted into the workspace.
pub const ARTIFACT_POISONING: Rule = Rule {
    id: "artifact-poisoning",
    summary: "Artifacts from other runs must not be extracted into the workspace.",
    tag: "artifact-poisoning",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[],
    configurable: false,
};

// GitHub dependency caching docs (write-capable `cache-mode` on low-trust triggers) and
// zizmor `cache-poisoning` (caches restored into release builds).
// https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#bypassing-the-default-untrusted-trigger-cache-restriction
pub const CACHE_POISONING: Rule = Rule {
    id: "cache-poisoning",
    summary: "Caches must not be writable from low-trust triggers or restored into release builds.",
    tag: "cache-poisoning",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "cache-poisoning")],
    configurable: false,
};

// zizmor `obfuscation`: paths and expressions written to hide what they do.
pub const OBFUSCATION: Rule = Rule {
    id: "obfuscation",
    summary: "`uses` paths and expressions must not hide what they reference.",
    tag: "obfuscation",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    equivalents: &[("zizmor", "obfuscation")],
    configurable: false,
};

pub fn script_injection(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let mut sinks = Vec::new();
            if let Some(script) = step.run() {
                sinks.push((step.key("run"), script));
            }
            if step.uses_action("actions/github-script")
                && let Some(script) = step.input_str("script")
            {
                sinks.push((step.key("with").with_key("script"), script));
            }
            for (route, script) in sinks {
                if let Some(context) = common::untrusted_in_text(script).first() {
                    findings.push(common::finding(
                        workflow,
                        &SCRIPT_INJECTION,
                        route,
                        step.location.clone(),
                        format!("untrusted `{context}` expands into a script"),
                    )?);
                }
            }
        }
    }
    Ok(findings)
}

pub fn env_injection(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let privileged = common::has_trigger(workflow, PRIVILEGED_TRIGGERS);
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some(script) = step.run() else { continue };
            // Step `env` overrides the job's, which overrides the workflow's.
            let mut tainted = std::collections::BTreeMap::new();
            for scope in [&workflow.value, job.value, step.value] {
                let Some(env) = scope.get("env").and_then(Value::as_mapping) else {
                    continue;
                };
                for (name, value) in env {
                    if let Some(name) = name.as_str() {
                        let untrusted = value
                            .as_str()
                            .is_some_and(|text| !common::untrusted_in_text(text).is_empty());
                        tainted.insert(name.to_ascii_lowercase(), untrusted);
                    }
                }
            }
            let unsafe_write = env_file_writes(script).iter().any(|written| {
                privileged
                    || !common::untrusted_in_text(written).is_empty()
                    || tainted
                        .iter()
                        .any(|(name, untrusted)| *untrusted && reads_variable(written, name))
            });
            if unsafe_write {
                findings.push(common::finding(
                    workflow,
                    &ENV_INJECTION,
                    step.key("run"),
                    step.location.clone(),
                    "writes GITHUB_ENV or GITHUB_PATH from an untrusted context",
                )?);
            }
        }
    }
    Ok(findings)
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
    let mut rest = line;
    while let Some(start) = rest.find("${{") {
        let after = &rest[start + 3..];
        let Some(end) = after.find("}}") else { break };
        out.push_str(&rest[..start]);
        match after[..end].trim() {
            "github.env" => out.push_str("$github_env"),
            "github.path" => out.push_str("$github_path"),
            _ => out.push_str(&rest[start..start + 3 + end + 2]),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
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

// ponytail: Known agent actions only; add a marketplace-category lookup when
// new agent actions appear faster than this list is maintained.
const AGENT_ACTIONS: &[&str] = &[
    "anthropics/claude-code-action",
    "anthropics/claude-code-base-action",
    "openai/codex-action",
    "google-github-actions/run-gemini-cli",
    "actions/ai-inference",
];

pub fn prompt_injection(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let external = common::has_trigger(workflow, EXTERNAL_TRIGGERS);
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            if !AGENT_ACTIONS.iter().any(|action| step.uses_action(action)) {
                continue;
            }
            let mut tainted = false;
            if let Some(inputs) = step.value.get("with") {
                common::strings(inputs, &Route::default(), &mut |_, text| {
                    tainted |= !common::untrusted_in_text(text).is_empty();
                });
            }
            if external || tainted {
                findings.push(common::finding(
                    workflow,
                    &PROMPT_INJECTION,
                    step.key("uses"),
                    step.location.clone(),
                    if tainted {
                        "untrusted event data reaches an agent prompt"
                    } else {
                        "agent runs on an event that outsiders can trigger"
                    },
                )?);
            }
        }
    }
    Ok(findings)
}

/// References to the pull request's code: its head (in the fork), the merge commit GitHub
/// builds from it, or the run's head for `workflow_run`.
const HEAD_REFS: &[&str] = &[
    "github.event.pull_request.head.",
    "github.event.pull_request.merge_commit_sha",
    "github.head_ref",
    "refs/pull/",
    "github.event.workflow_run.head_",
];

/// Whether a step checks out or fetches the pull request's code, which the author controls.
/// Any ref of the fork repository counts, so `repository` alone is enough. Context paths are
/// compared as `untrusted_contexts` reads them: lowercased, with index syntax made dotted.
fn checks_out_head(step: &common::Step) -> bool {
    let mentions_head = |text: &str| {
        let text = common::dotted(&text.to_ascii_lowercase());
        HEAD_REFS.iter().any(|head| text.contains(head))
    };
    let checkout = step
        .uses()
        .is_some_and(|uses| uses.to_ascii_lowercase().starts_with("actions/checkout@"))
        && ["ref", "repository"]
            .iter()
            .any(|input| step.input_str(input).is_some_and(mentions_head));
    let fetch = step.run().is_some_and(|script| {
        script
            .replace("\\\n", " ")
            .split(['\n', ';', '&', '|'])
            .any(|command| {
                command.contains("gh pr checkout")
                    || git_arguments(command, &["fetch", "pull", "clone", "checkout", "switch"])
                        .is_some_and(|arguments| {
                            arguments.contains("pull/") || mentions_head(&arguments)
                        })
            })
    });
    checkout || fetch
}

/// Global options of `git` that take their value as the next word:
/// https://git-scm.com/docs/git#_options
const GIT_OPTIONS_WITH_VALUE: &[&str] = &[
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--config-env",
];

/// The arguments after the subcommand when `command` runs `git` with one of `subcommands`,
/// skipping variable assignments and wrappers before `git` and its global options after it.
fn git_arguments(command: &str, subcommands: &[&str]) -> Option<String> {
    let mut words = command
        .split_whitespace()
        .skip_while(|word| {
            ["sudo", "command", "exec", "env", "time"].contains(word)
                || (word.contains('=') && !word.starts_with('-'))
        })
        .peekable();
    words.next().filter(|word| *word == "git")?;
    while let Some(option) = words.next_if(|word| word.starts_with('-')) {
        if GIT_OPTIONS_WITH_VALUE.contains(&option) {
            words.next();
        }
    }
    words
        .next()
        .filter(|subcommand| subcommands.contains(subcommand))?;
    Some(words.collect::<Vec<_>>().join(" "))
}

pub fn pwn_request(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    if !common::has_trigger(workflow, PRIVILEGED_TRIGGERS) {
        return Ok(Vec::new());
    }
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            if checks_out_head(&step) {
                findings.push(common::finding(
                    workflow,
                    &PWN_REQUEST,
                    step.anchor(),
                    step.location.clone(),
                    "privileged trigger checks out untrusted pull request code",
                )?);
            }
        }
    }
    Ok(findings)
}

pub fn dangerous_trigger(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    common::triggers(workflow)
        .into_iter()
        .filter(|trigger| PRIVILEGED_TRIGGERS.contains(&trigger.name))
        .map(|trigger| {
            common::finding(
                workflow,
                &DANGEROUS_TRIGGER,
                trigger.route,
                "workflow",
                format!("`{}` runs with base repository privileges", trigger.name),
            )
        })
        .collect()
}

pub fn unsafe_if(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut conditions = Vec::new();
    for job in common::jobs(workflow) {
        if let Some(condition) = job.get("if") {
            conditions.push((job.route.with_key("if"), job.location(), condition));
        }
        for step in common::steps(&job) {
            if let Some(condition) = step.value.get("if") {
                conditions.push((step.key("if"), step.location.clone(), condition));
            }
        }
    }
    let mut findings = Vec::new();
    for (route, location, condition) in conditions {
        let problem = match condition {
            Value::Bool(value) => Some((format!("condition is always {value}"), None)),
            Value::String(text) if text.contains("${{") && !single_expression(text) => Some((
                "text around `${{ }}` makes the condition always true".to_owned(),
                Some("unsound-condition"),
            )),
            Value::String(text) if trusts_bot_actor(text) => Some((
                "actor name checks can be spoofed by a bot-triggered event".to_owned(),
                Some("bot-conditions"),
            )),
            _ => None,
        };
        if let Some((message, concept)) = problem {
            let mut finding = common::finding(workflow, &UNSAFE_IF, route, location, message)?;
            finding.concept = concept;
            findings.push(finding);
        }
    }
    Ok(findings)
}

/// Whether `condition` runs something because the actor is a bot: `==`, `contains`,
/// `startsWith`, or `endsWith` matching `github.actor` or `github.triggering_actor` with a
/// `[bot]` name. A spoofed actor fails a `!=` or negated check, which only skips the work; a
/// check is negated by an odd number of `!` or `== false` around its enclosing groups.
fn trusts_bot_actor(condition: &str) -> bool {
    let text = condition.to_ascii_lowercase();
    ["github.actor", "github.triggering_actor"]
        .iter()
        .flat_map(|actor| {
            text.match_indices(actor)
                .map(|(at, actor)| (at, at + actor.len()))
        })
        .filter(|&(_, end)| {
            !text[end..]
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
        .any(|(start, end)| {
            let before = text[..start].trim_end();
            let after = text[end..].trim_start();
            let positive = negations(&text, start).is_multiple_of(2);
            if let Some(operand) = after.strip_prefix("==") {
                return positive && leading_literal(operand.trim_start()).contains("[bot]");
            }
            if let Some(operand) = before.strip_suffix("==") {
                let operand = operand.trim_end();
                let literal = operand
                    .strip_suffix('\'')
                    .and_then(|rest| rest.rfind('\'').map(|open| &rest[open..]));
                return positive && literal.is_some_and(|literal| literal.contains("[bot]"));
            }
            if after.starts_with("!=") || before.ends_with("!=") {
                return false;
            }
            positive
                && enclosing_call(&text, start).is_some_and(|(name, call)| {
                    ["contains", "startswith", "endswith"].contains(&name) && call.contains("[bot]")
                })
        })
}

/// How many of the parenthesized groups and calls around `text[at]` are negated by a leading
/// `!`, a trailing `== false` or `!= true`, or a leading `false ==` or `true !=`.
fn negations(text: &str, mut at: usize) -> usize {
    let mut count = 0;
    while let Some((open, close)) = enclosing_group(text, at) {
        let head = text[..open].trim_end();
        let name_start = head
            .rfind(|char: char| !(char.is_ascii_alphanumeric() || char == '_'))
            .map_or(0, |index| index + 1);
        let before = head[..name_start].trim_end();
        let after = text[close + 1..].trim_start();
        let compared_false = ["== false", "!= true"]
            .iter()
            .any(|suffix| after.starts_with(suffix))
            || ["false ==", "true !="]
                .iter()
                .any(|prefix| before.ends_with(prefix));
        count += usize::from(before.ends_with('!')) + usize::from(compared_false);
        at = name_start;
    }
    count
}

/// Byte offsets of the `(` and `)` of the innermost group around `text[at]`.
fn enclosing_group(text: &str, at: usize) -> Option<(usize, usize)> {
    let mut depth = 0;
    let open = text[..at].char_indices().rev().find_map(|(index, char)| {
        match char {
            ')' => depth += 1,
            '(' if depth == 0 => return Some(index),
            '(' => depth -= 1,
            _ => {}
        }
        None
    })?;
    let mut depth = 0;
    let close = text[open..].char_indices().find_map(|(index, char)| {
        match char {
            '(' => depth += 1,
            ')' if depth == 1 => return Some(open + index),
            ')' => depth -= 1,
            _ => {}
        }
        None
    })?;
    Some((open, close))
}

/// The single-quoted literal that `text` starts with, or "" when it starts with something else.
fn leading_literal(text: &str) -> &str {
    text.strip_prefix('\'')
        .and_then(|rest| rest.find('\'').map(|close| &rest[..close]))
        .unwrap_or("")
}

/// Name and full text of the innermost function call around `text[at]`.
fn enclosing_call(text: &str, at: usize) -> Option<(&str, &str)> {
    let (open, close) = enclosing_group(text, at)?;
    let head = text[..open].trim_end();
    let name_start = head
        .rfind(|char: char| !(char.is_ascii_alphanumeric() || char == '_'))
        .map_or(0, |index| index + 1);
    Some((&head[name_start..], &text[name_start..=close]))
}

fn single_expression(text: &str) -> bool {
    text.starts_with("${{") && text.ends_with("}}") && text.matches("${{").count() == 1
}

pub fn artifact_poisoning(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    if !common::has_trigger(workflow, &["workflow_run"]) {
        return Ok(Vec::new());
    }
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let cross_run = (step.uses_action("actions/download-artifact")
                && step.input("run-id").is_some())
                || step.uses_action("dawidd6/action-download-artifact");
            let outside_workspace = step.input_str("path").is_some_and(|path| {
                let path = path.replace(' ', "");
                path.starts_with("${{runner.temp}}") || path.starts_with("/tmp")
            });
            if cross_run && !outside_workspace {
                findings.push(common::finding(
                    workflow,
                    &ARTIFACT_POISONING,
                    step.key("uses"),
                    step.location.clone(),
                    "artifact from another run is extracted into the workspace",
                )?);
            }
        }
    }
    Ok(findings)
}

/// Triggers that may write caches in the default branch scope; `pull_request` caches are
/// scoped to the merge ref. `workflow_call` inherits its caller's trust.
const CACHE_TRUSTED_TRIGGERS: &[&str] = &[
    "push",
    "workflow_dispatch",
    "repository_dispatch",
    "delete",
    "registry_package",
    "page_build",
    "schedule",
    "pull_request",
    "workflow_call",
];

pub fn cache_poisoning(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let triggers = common::triggers(workflow);
    let mut findings = Vec::new();

    let low_trust = triggers
        .iter()
        .any(|trigger| !CACHE_TRUSTED_TRIGGERS.contains(&trigger.name));
    if low_trust {
        let mut scopes = vec![(Route::default(), "workflow".to_owned(), &workflow.value)];
        for job in common::jobs(workflow) {
            scopes.push((job.route.clone(), job.location(), job.value));
        }
        for (route, location, scope) in scopes {
            if let Some(mode @ ("write" | "write-only")) =
                scope.get("cache-mode").and_then(Value::as_str)
            {
                findings.push(common::finding(
                    workflow,
                    &CACHE_POISONING,
                    route.with_key("cache-mode"),
                    location,
                    format!("`cache-mode: {mode}` lets a low-trust trigger write caches"),
                )?);
            }
        }
        // Without an explicit cap on the caller, a called workflow may request `write`:
        // https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#cache-access-in-reusable-workflows
        // Only outsider triggers count; maintainer-only ones such as `release` would be noise.
        if common::has_trigger(workflow, EXTERNAL_TRIGGERS)
            && workflow.value.get("cache-mode").is_none()
        {
            for job in common::jobs(workflow) {
                if job.get("uses").is_some() && job.get("cache-mode").is_none() {
                    findings.push(common::finding(
                        workflow,
                        &CACHE_POISONING,
                        job.route.with_key("uses"),
                        job.location(),
                        "called workflow can request cache writes on a low-trust trigger; set `cache-mode: read`",
                    )?);
                }
            }
        }
    }

    // A privileged run shares the base branch's cache scope, so a cache saved after the pull
    // request's code ran is restored by later trusted runs:
    // https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#restrictions-for-accessing-a-cache
    if common::has_trigger(workflow, PRIVILEGED_TRIGGERS) {
        for job in common::jobs(workflow) {
            let steps = common::steps(&job);
            let Some(checkout) = steps.iter().position(checks_out_head) else {
                continue;
            };
            let saves = steps
                .iter()
                .enumerate()
                .filter(|(index, step)| saves_cache(workflow, step, *index > checkout));
            for (_, step) in saves {
                findings.push(common::finding(
                    workflow,
                    &CACHE_POISONING,
                    step.key("uses"),
                    step.location.clone(),
                    "cache saved after pull request code ran on a privileged trigger reaches the base branch",
                )?);
            }
        }
    }

    let release = triggers.iter().any(|trigger| {
        trigger.name == "release"
            || (trigger.name == "push"
                && trigger
                    .config
                    .is_some_and(|config| config.get("tags").is_some()))
    });
    if release {
        for job in common::jobs(workflow) {
            for step in common::steps(&job) {
                if uses_cache(workflow, &step) {
                    findings.push(common::finding(
                        workflow,
                        &CACHE_POISONING,
                        step.key("uses"),
                        step.location.clone(),
                        "release build restores a cache that other runs can write",
                    )?);
                }
            }
        }
    }
    Ok(findings)
}

/// Whether the step writes a cache once the job has run. `actions/cache` and the setup actions
/// save in their `post` step at the end of the job (action.yml of each); `actions/cache/save`
/// saves where it stands, and `actions/cache/restore` never saves.
fn saves_cache(workflow: &Workflow, step: &common::Step, after_checkout: bool) -> bool {
    let name = step
        .uses()
        .map(|uses| uses.split('@').next().unwrap_or("").to_ascii_lowercase());
    match name.as_deref() {
        Some("actions/cache/save") => after_checkout,
        Some("actions/cache/restore") => false,
        _ => uses_cache(workflow, step),
    }
}

/// Whether the step restores a cache.
fn uses_cache(workflow: &Workflow, step: &common::Step) -> bool {
    let Some(uses) = step.uses() else {
        return false;
    };
    let name = uses.split('@').next().unwrap_or("").to_ascii_lowercase();
    let cache_input = step.input("cache");
    let disabled = |value: Option<&Value>| {
        matches!(value, Some(Value::Bool(false)))
            || value
                .and_then(Value::as_str)
                .is_some_and(|text| text.is_empty() || text == "false")
    };
    match name.as_str() {
        "actions/cache" | "actions/cache/restore" => true,
        // setup-go's `cache` defaults to true from v4 and to false before (action.yml of each tag).
        "actions/setup-go" => {
            let before_v4 = maintainability::versioned_action(workflow, step)
                .is_some_and(|(_, version)| version.first().is_some_and(|major| *major < 4));
            if before_v4 {
                cache_input.is_some() && !disabled(cache_input)
            } else {
                !disabled(cache_input)
            }
        }
        // setup-uv's `enable-cache` defaults to false before v5, then to `auto`, which from v10 skips
        // release, tag push, pull_request_target, and workflow_run runs (action.yml of each tag).
        "astral-sh/setup-uv" => {
            let major = maintainability::versioned_action(workflow, step)
                .and_then(|(_, version)| version.first().copied());
            match step.input("enable-cache") {
                Some(Value::Bool(enabled)) => *enabled,
                Some(Value::String(text)) if text != "auto" => text == "true",
                _ => major.is_some_and(|major| (5..10).contains(&major)),
            }
        }
        "actions/setup-node"
        | "actions/setup-python"
        | "actions/setup-java"
        | "actions/setup-dotnet" => cache_input.is_some() && !disabled(cache_input),
        _ => false,
    }
}

pub fn obfuscation(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for (route, location, uses) in common::all_uses(workflow) {
        let name = uses.rsplit_once('@').map_or(uses, |(name, _)| name);
        let odd_path = name.contains("//")
            || name
                .split('/')
                .any(|segment| segment == "." || segment == "..")
            || name.ends_with('/');
        let local_or_image = ["./", "$/", "docker://"]
            .iter()
            .any(|prefix| uses.starts_with(prefix));
        if odd_path && !local_or_image {
            findings.push(common::finding(
                workflow,
                &OBFUSCATION,
                route,
                location,
                "`uses` path contains redundant segments",
            )?);
        }
    }

    let mut routes = BTreeSet::new();
    let mut flagged = Vec::new();
    common::strings(&workflow.value, &Route::default(), &mut |route, text| {
        let obfuscated = common::expressions(text)
            .into_iter()
            .any(|expression| is_string_literal(expression) || expression.contains("github["));
        if obfuscated && routes.insert(format!("{route:?}")) {
            flagged.push(route.clone());
        }
    });
    for route in flagged {
        findings.push(common::finding(
            workflow,
            &OBFUSCATION,
            route,
            "workflow",
            "expression hides a literal or a context path",
        )?);
    }
    Ok(findings)
}

fn is_string_literal(expression: &str) -> bool {
    expression.len() >= 2
        && expression.starts_with('\'')
        && expression.ends_with('\'')
        && !expression[1..expression.len() - 1].contains('\'')
}
