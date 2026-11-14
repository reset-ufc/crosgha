//! `gh` commands in `run` steps that lack the token, host, or repository they need, after ghlint's
//! `MissingGhToken`, `MissingGhHost`, and `MissingGhRepo` at commit 74115a4:
//! https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/MissingGhTokenRule.kt
//! https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/MissingGhRepoRule.kt
//!
//! The variables and their precedence come from `gh help environment`, and the commands that
//! need them from gh's source, both at cli/cli v2.104.0 (3d5a9e355a837f2f5ee78d7484c118cc1342ad1a):
//! `pkg/cmd/root/help_topic.go`, `pkg/cmd/root/root.go`, `cmdutil.DisableAuthCheck`, and
//! `cmdutil.EnableRepoOverride`.

use std::collections::BTreeSet;

use yaml_serde::Value;

use super::{Finding, Rule, common, shell};
use crate::policy::{Persona, Severity};
use crate::workflow::Workflow;

pub const RULE: Rule = Rule {
    id: "gh-cli-environment",
    summary: "Steps that run `gh` must set its token, and a repository for commands that read one.",
    tag: "embedded-script-lint",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[
        ("ghlint", "MissingGhToken"),
        ("ghlint", "MissingGhHost"),
        ("ghlint", "MissingGhRepo"),
    ],
    configurable: false,
    relational: false,
};

/// Tokens for github.com, `*.ghe.com`, and `github.localhost`, in order of precedence.
const TOKENS: &[&str] = &["GH_TOKEN", "GITHUB_TOKEN"];
/// Tokens for any other host, a GitHub Enterprise Server, which `GH_HOST` names.
const ENTERPRISE_TOKENS: &[&str] = &["GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"];
const HOST: &[&str] = &["GH_HOST"];
/// `[HOST/]OWNER/REPO` for commands that otherwise read the repository in the working directory.
const REPOSITORY: &[&str] = &["GH_REPO"];

/// Shells whose scripts run `gh` as a command; Python or Node.js scripts call it otherwise.
const COMMAND_SHELLS: &[&str] = &["bash", "sh", "pwsh", "powershell", "cmd"];

/// Top-level commands, with their aliases, that gh runs only with a token. The others skip the
/// check (`auth`, `config`, `version`, `alias`, `extension install`, extensions, help topics) or
/// are left out (`extension`, `skill`, which skip it for some options).
const AUTHENTICATED: &[&str] = &[
    "agent",
    "agent-task",
    "agent-tasks",
    "agents",
    "api",
    "at",
    "attestation",
    "browse",
    "cache",
    "codespace",
    "cs",
    "discussion",
    "gist",
    "gpg-key",
    "issue",
    "label",
    "org",
    "pr",
    "project",
    "release",
    "repo",
    "rs",
    "ruleset",
    "run",
    "search",
    "secret",
    "ssh-key",
    "status",
    "variable",
    "workflow",
];

/// Top-level commands that read the repository from `--repo`, `GH_REPO`, or the git remote of the
/// working directory.
const REPOSITORY_COMMANDS: &[&str] = &[
    "browse",
    "cache",
    "discussion",
    "issue",
    "label",
    "pr",
    "release",
    "rs",
    "ruleset",
    "run",
    "secret",
    "variable",
    "workflow",
];

/// `gh repo` subcommands that act on the current repository rather than on an operand.
const REPOSITORY_SUBCOMMANDS: &[&str] =
    &["autolink", "deploy-key", "read-dir", "read-file", "rename"];

/// Commands whose operand may be a URL, which names the repository.
const URL_COMMANDS: &[&str] = &["discussion", "issue", "pr"];

/// `gh api` placeholders filled from the current repository or `GH_REPO`.
const PLACEHOLDERS: &[&str] = &["{owner}", "{repo}", "{branch}"];

/// What a `gh` command lacks; each part is one of ghlint's IDs.
#[derive(Clone, Copy, PartialEq)]
enum Missing {
    Token,
    Host,
    Repository,
}

impl Missing {
    fn concepts(self) -> &'static [&'static str] {
        match self {
            Self::Token => &["MissingGhToken"],
            Self::Host => &["MissingGhHost"],
            Self::Repository => &["MissingGhRepo"],
        }
    }
}

/// What the environment gives `gh` at one point of a job.
#[derive(Clone, Default)]
struct Context<'a> {
    /// Environment variables, which `gh` inherits.
    variables: BTreeSet<&'a str>,
    /// Shell variables the script assigned without exporting them, so `gh` does not see them
    /// until `export NAME`.
    unexported: BTreeSet<&'a str>,
    /// An `env` is an expression, so it may set any variable.
    unknown: bool,
    /// `gh auth login` stored a token that later commands use.
    logged_in: bool,
    /// actions/checkout, `git clone`, or `gh repo clone` ran.
    repository: bool,
}

impl<'a> Context<'a> {
    fn set(&mut self, env: Option<&'a Value>) {
        match env {
            Some(Value::Mapping(env)) => {
                self.variables.extend(env.keys().filter_map(Value::as_str))
            }
            Some(Value::String(_)) => self.unknown = true,
            _ => {}
        }
    }

    /// Records the variables that `words` export: `NAME=value`, as after `export` or before a
    /// program, and `NAME` alone when the script assigned it before.
    fn export(&mut self, words: &[&'a str]) {
        for word in words {
            match word.split_once('=') {
                Some((name, _)) if shell::is_assignment(word) => {
                    self.variables.insert(name);
                }
                None if self.unexported.contains(word) => {
                    self.variables.insert(word);
                }
                _ => {}
            }
        }
    }

    /// Records the shell variables that `words` assign without exporting them.
    fn keep(&mut self, words: &[&'a str]) {
        for word in words {
            if shell::is_assignment(word)
                && let Some((name, _)) = word.split_once('=')
            {
                self.unexported.insert(name);
            }
        }
    }

    fn sees(&self, names: &[&str]) -> bool {
        self.unknown || names.iter().any(|name| self.variables.contains(name))
    }
}

pub fn detect(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        // A container job's steps run in the container, which also gets `container.env`.
        let container = job
            .get("container")
            .and_then(|container| container.get("env"));
        let mut context = Context::default();
        for env in [workflow.value.get("env"), job.get("env"), container] {
            context.set(env);
        }
        for step in common::steps(&job) {
            if step.uses_action("actions/checkout") {
                context.repository = true;
            }
            let Some(script) = step.run() else { continue };
            let shell = shell::name(workflow, &job, &step).unwrap_or_default();
            if COMMAND_SHELLS.contains(&shell) {
                let mut seen = context.clone();
                seen.set(step.value.get("env"));
                for (missing, message) in problems(script, shell, &mut seen) {
                    let mut finding = common::finding(
                        workflow,
                        &RULE,
                        step.key("run"),
                        step.location.clone(),
                        message,
                    )?;
                    finding.concepts = missing.concepts();
                    findings.push(finding);
                }
                context.logged_in = seen.logged_in;
                context.repository = seen.repository;
            }
            context.variables.extend(environment_file_writes(script));
        }
    }
    Ok(findings)
}

/// The first `gh` command of `script`, run by `shell`, that lacks each part of its environment,
/// with the message. `context` takes the variables the script sets and what its commands leave
/// for later steps.
fn problems<'a>(script: &'a str, shell: &str, context: &mut Context<'a>) -> Vec<(Missing, String)> {
    let mut found: Vec<(Missing, String)> = Vec::new();
    let powershell = matches!(shell, "pwsh" | "powershell");
    for command in shell::commands(script) {
        let Some(program) = shell::program(&command.words) else {
            context.keep(&command.words);
            continue;
        };
        if powershell && let Some(name) = powershell_variable(program.path) {
            context.variables.insert(name);
            continue;
        }
        match program.name {
            "export" => context.export(program.args),
            // `declare -x` and `typeset -x` export; without it they, `local`, and `readonly` only
            // assign.
            "declare" | "typeset"
                if program
                    .args
                    .iter()
                    .any(|arg| arg.starts_with('-') && arg.contains('x')) =>
            {
                context.export(program.args);
            }
            "declare" | "typeset" | "local" | "readonly" => context.keep(program.args),
            // cmd has no shell variables: `set NAME=value` changes the environment. In bash, `set`
            // sets positional parameters.
            "set" if shell == "cmd" => context.export(program.args),
            "git" if shell::git_subcommand(&program).is_some_and(|(name, _)| name == "clone") => {
                context.repository = true;
            }
            "gh" => {
                // Assignments before `gh`, also through `env` or `sudo`, set variables for it alone.
                let before = &command.words[..command.words.len() - program.args.len() - 1];
                let mut seen = context.clone();
                seen.export(before);
                for (missing, message) in lacking(program.args, &seen) {
                    if !found.iter().any(|(other, _)| *other == missing) {
                        found.push((missing, message));
                    }
                }
                match program.args {
                    ["auth", "login", ..] => context.logged_in = true,
                    ["repo", "clone", ..] => context.repository = true,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    found
}

/// The name PowerShell's `$env:NAME = value` sets, from its first word.
fn powershell_variable(word: &str) -> Option<&str> {
    let prefix = word.get(..5)?;
    let rest = &word[5..];
    let name = rest.split_once('=').map_or(rest, |(name, _)| name);
    (prefix.eq_ignore_ascii_case("$env:") && !name.is_empty()).then_some(name)
}

/// What `gh <args>` needs and `seen` does not give it.
fn lacking(args: &[&str], seen: &Context) -> Vec<(Missing, String)> {
    let mut missing = Vec::new();
    let command = invocation(args);
    if needs_token(args) && !seen.logged_in && !seen.sees(TOKENS) {
        match (seen.sees(ENTERPRISE_TOKENS), seen.sees(HOST)) {
            (true, true) => {}
            (true, false) if !has_option(args, None, "hostname") => missing.push((
                Missing::Host,
                format!(
                    "`{command}` sees `GH_ENTERPRISE_TOKEN` but no `GH_HOST`, so it targets \
                     github.com: set `GH_HOST` to the Enterprise Server host"
                ),
            )),
            (true, false) => {}
            (false, true) => missing.push((
                Missing::Token,
                format!(
                    "`{command}` sees `GH_HOST` but no `GH_ENTERPRISE_TOKEN`: set the host's \
                     token in `env`"
                ),
            )),
            (false, false) => missing.push((
                Missing::Token,
                format!(
                    "`{command}` runs without a token: set `GH_TOKEN: ${{{{ github.token }}}}` in \
                     `env`"
                ),
            )),
        }
    }
    if needs_repository(args) && !seen.repository && !seen.sees(REPOSITORY) {
        missing.push((
            Missing::Repository,
            format!(
                "`{command}` runs without a repository: check out the code first, set \
                 `GH_REPO: ${{{{ github.repository }}}}` in `env`, or pass `--repo`"
            ),
        ));
    }
    missing
}

/// `gh` and its command words, without options and operands: `gh pr list`.
fn invocation(args: &[&str]) -> String {
    let words: Vec<&str> = args
        .iter()
        .take(2)
        .take_while(|arg| !arg.starts_with('-') && !arg.contains('$'))
        .copied()
        .collect();
    format!("gh {}", words.join(" "))
}

/// Whether `args` hold `--long`, `--long=value`, or, with `short`, a `-x` option cluster starting
/// with that letter.
fn has_option(args: &[&str], short: Option<char>, long: &str) -> bool {
    args.iter().any(|arg| match arg.strip_prefix("--") {
        Some(option) => option.split('=').next() == Some(long),
        None => short.is_some_and(|short| {
            arg.strip_prefix('-')
                .is_some_and(|cluster| cluster.starts_with(short))
        }),
    })
}

/// Whether `args` run a command, rather than `--version` or help.
fn command<'a>(args: &[&'a str]) -> Option<(&'a str, Option<&'a str>)> {
    let (&name, rest) = args.split_first()?;
    let help = has_option(args, Some('h'), "help");
    (!name.starts_with('-') && !help).then(|| (name, rest.first().copied()))
}

fn needs_token(args: &[&str]) -> bool {
    match command(args) {
        // `release download` skips the check to read public repositories anonymously, and
        // attestations verify offline from a trusted root or a bundle file.
        Some(("release", Some("download")) | ("at" | "attestation", Some("trusted-root"))) => false,
        Some(("at" | "attestation", Some("verify"))) => !has_option(args, Some('b'), "bundle"),
        Some((name, _)) => AUTHENTICATED.contains(&name),
        None => false,
    }
}

fn needs_repository(args: &[&str]) -> bool {
    let Some((name, subcommand)) = command(args) else {
        return false;
    };
    if has_option(args, Some('R'), "repo") {
        return false;
    }
    match name {
        "api" => args.iter().any(|arg| {
            PLACEHOLDERS
                .iter()
                .any(|placeholder| arg.contains(placeholder))
        }),
        "repo" => subcommand.is_some_and(|subcommand| REPOSITORY_SUBCOMMANDS.contains(&subcommand)),
        // Organization and user secrets, variables, and rulesets belong to no repository.
        "secret" | "variable" | "rs" | "ruleset"
            if has_option(args, Some('o'), "org") || has_option(args, Some('u'), "user") =>
        {
            false
        }
        // An operand known only at run time may be a URL: `gh pr merge "$PR_URL"`.
        name if URL_COMMANDS.contains(&name)
            && args[1..]
                .iter()
                .any(|arg| arg.contains("://") || arg.contains('$')) =>
        {
            false
        }
        name => REPOSITORY_COMMANDS.contains(&name),
    }
}

/// The variables `script` writes to `GITHUB_ENV` for later steps, read loosely: the script names
/// the file and holds `NAME=` or `NAME<<` for a heredoc value.
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#setting-an-environment-variable
fn environment_file_writes(script: &str) -> Vec<&'static str> {
    if !script.contains("GITHUB_ENV") {
        return Vec::new();
    }
    let names = TOKENS
        .iter()
        .chain(ENTERPRISE_TOKENS)
        .chain(HOST)
        .chain(REPOSITORY);
    names
        .filter(|name| {
            script.contains(&format!("{name}=")) || script.contains(&format!("{name}<<"))
        })
        .copied()
        .collect()
}
