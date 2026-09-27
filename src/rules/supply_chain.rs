use super::{Finding, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// Khatami et al. (SCAM 2024), KZ-20 "Specify package versions", and zizmor's
// `adhoc-packages`/`unpinned-tools` audits: https://docs.zizmor.sh/audits/
pub const UNPINNED_PACKAGE: Rule = Rule {
    id: "unpinned-package",
    tag: "unpinned-package",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    aliases: &["adhoc-packages", "unpinned-tools"],
};

// Poutine `unverified_script_exec` and Semgrep `curl-eval`: a downloaded script runs
// before anything checks its integrity.
pub const REMOTE_SCRIPT: Rule = Rule {
    id: "unverified-remote-script",
    tag: "unverified-remote-script",
    severity: Severity::Warning,
    persona: Persona::Regular,
    aliases: &[],
};

// zizmor `typosquat-uses`: an owner one edit away from a well-known publisher.
pub const TYPOSQUATTING: Rule = Rule {
    id: "typosquatted-action",
    tag: "typosquatting/impostor-commit",
    severity: Severity::Error,
    persona: Persona::Regular,
    aliases: &["typosquat-uses"],
};

pub fn unpinned_package(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some(script) = step.run() else { continue };
            if let Some(command) = commands(script).find(|words| unpinned_install(words)) {
                findings.push(common::finding(
                    workflow,
                    &UNPINNED_PACKAGE,
                    step.key("run"),
                    step.location.clone(),
                    format!(
                        "`{}` installs a package without a version",
                        command.join(" ")
                    ),
                )?);
            }
        }
    }
    Ok(findings)
}

/// Shell commands split on newlines and control operators, without `sudo` and env assignments.
fn commands(script: &str) -> impl Iterator<Item = Vec<&str>> {
    script
        .split(['\n', ';', '|', '&'])
        .map(|command| {
            command
                .split_whitespace()
                .skip_while(|word| *word == "sudo" || word.contains('=') && !word.starts_with('-'))
                .collect::<Vec<_>>()
        })
        .filter(|words| !words.is_empty())
}

fn unpinned_install(words: &[&str]) -> bool {
    let words: Vec<&str> = match words {
        ["python" | "python3", "-m", rest @ ..] => rest.to_vec(),
        ["uv", "pip", rest @ ..] => [&["pip"][..], rest].concat(),
        ["uv", "tool", rest @ ..] => [&["pipx"][..], rest].concat(),
        _ => words.to_vec(),
    };
    match words.as_slice() {
        ["pip" | "pip3" | "pipx", "install", args @ ..] => arguments(
            args,
            &[
                "-r",
                "--requirement",
                "-c",
                "--constraint",
                "-e",
                "--editable",
                "-i",
                "--index-url",
                "--extra-index-url",
                "-f",
                "--find-links",
                "-t",
                "--target",
            ],
        )
        .any(|package| !package.contains("==") && !is_local_or_url(package)),
        ["npm" | "pnpm", "install" | "i" | "add", args @ ..]
        | ["yarn", "global", "add", args @ ..] => arguments(args, &["--prefix", "--registry"])
            .any(|package| !has_npm_version(package) && !is_local_or_url(package)),
        ["go", "install", args @ ..] => arguments(args, &["-C", "-o"]).any(|module| {
            !is_local_or_url(module)
                && module
                    .rsplit_once('@')
                    .is_none_or(|(_, version)| version == "latest")
        }),
        ["cargo", "install", args @ ..] => {
            let pinned_by_flag = args
                .iter()
                .any(|arg| matches!(*arg, "--version" | "--vers" | "--path" | "--git"));
            !pinned_by_flag
                && arguments(
                    args,
                    &[
                        "--root",
                        "--features",
                        "-F",
                        "--target",
                        "-j",
                        "--jobs",
                        "--profile",
                        "--bin",
                        "--example",
                        "--registry",
                        "--index",
                        "--branch",
                        "--tag",
                        "--rev",
                    ],
                )
                .any(|name| !name.contains('@'))
        }
        ["gem", "install", args @ ..] => {
            !args.iter().any(|arg| matches!(*arg, "-v" | "--version"))
                && arguments(args, &["--source", "-s", "--install-dir", "-i"])
                    .any(|name| !name.contains(':'))
        }
        _ => false,
    }
}

/// Positional arguments, skipping flags and the values of `valued` flags.
fn arguments<'a>(args: &'a [&'a str], valued: &'a [&'a str]) -> impl Iterator<Item = &'a str> + 'a {
    let mut skip_next = false;
    args.iter().copied().filter(move |arg| {
        if std::mem::take(&mut skip_next) {
            return false;
        }
        if arg.starts_with('-') {
            skip_next = valued.contains(arg);
            return false;
        }
        !arg.contains("${{") && !arg.starts_with('$') && !arg.starts_with('>')
    })
}

fn is_local_or_url(argument: &str) -> bool {
    argument.starts_with(['.', '/', '~'])
        || argument.contains("://")
        || argument.starts_with("git+")
        || argument.starts_with("file:")
        || [".whl", ".tar.gz", ".zip", ".tgz"]
            .iter()
            .any(|suffix| argument.ends_with(suffix))
}

fn has_npm_version(package: &str) -> bool {
    package.rfind('@').is_some_and(|index| index > 0)
}

pub fn remote_script(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some(script) = step.run() else { continue };
            if script.lines().any(pipes_download_to_interpreter) {
                findings.push(common::finding(
                    workflow,
                    &REMOTE_SCRIPT,
                    step.key("run"),
                    step.location.clone(),
                    "downloaded script runs without an integrity check",
                )?);
            }
        }
    }
    Ok(findings)
}

const INTERPRETERS: &[&str] = &[
    "sh", "bash", "zsh", "dash", "ksh", "python", "python3", "perl", "ruby", "node",
];

fn pipes_download_to_interpreter(line: &str) -> bool {
    let downloads = |text: &str| text.contains("curl") || text.contains("wget");
    if line.contains("<(curl") || line.contains("<(wget") {
        return true;
    }
    if (line.contains("$(curl") || line.contains("$(wget"))
        && ["sh -c", "bash -c", "eval"]
            .iter()
            .any(|runner| line.contains(runner))
    {
        return true;
    }
    let mut segments = line.split('|');
    let Some(first) = segments.next() else {
        return false;
    };
    let mut downloaded = downloads(first);
    for segment in segments {
        let program = segment
            .split_whitespace()
            .find(|word| *word != "sudo" && !word.starts_with('-'))
            .unwrap_or("");
        if downloaded && INTERPRETERS.contains(&program.rsplit('/').next().unwrap_or(program)) {
            return true;
        }
        downloaded = downloaded || downloads(segment);
    }
    false
}

// ponytail: A fixed list of high-traffic publishers; load a curated
// popularity list when a larger corpus justifies it.
const KNOWN_OWNERS: &[&str] = &[
    "actions",
    "github",
    "docker",
    "aws-actions",
    "azure",
    "google-github-actions",
    "hashicorp",
    "gradle",
    "pypa",
    "codecov",
    "step-security",
    "anthropics",
];

/// Legitimate publishers one edit away from a known owner, seen in the benchmark corpus.
const KNOWN_NEIGHBORS: &[&str] = &["actionsx"];

pub fn typosquatting(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for (route, location, uses) in common::all_uses(workflow) {
        let Some(reference) = common::parse_uses(uses) else {
            continue;
        };
        let owner = reference.owner.to_ascii_lowercase();
        if KNOWN_NEIGHBORS.contains(&owner.as_str()) {
            continue;
        }
        if let Some(known) = KNOWN_OWNERS
            .iter()
            .find(|known| **known != owner && one_edit_apart(&owner, known))
        {
            findings.push(common::finding(
                workflow,
                &TYPOSQUATTING,
                route,
                location,
                format!(
                    "owner `{}` is one edit away from `{known}`",
                    reference.owner
                ),
            )?);
        }
    }
    Ok(findings)
}

/// Optimal string alignment distance of exactly one: an insertion, deletion,
/// substitution, or adjacent transposition.
fn one_edit_apart(left: &str, right: &str) -> bool {
    let (a, b) = (left.as_bytes(), right.as_bytes());
    if a == b || a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[prefix..], &b[prefix..]);
    match a.len().cmp(&b.len()) {
        std::cmp::Ordering::Equal => {
            a[1..] == b[1..] || (a.len() >= 2 && a[0] == b[1] && a[1] == b[0] && a[2..] == b[2..])
        }
        std::cmp::Ordering::Less => a == &b[1..],
        std::cmp::Ordering::Greater => &a[1..] == b,
    }
}
