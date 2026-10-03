use yaml_serde::Value;

use super::{Finding, Options, Rule, common};
use crate::{
    policy::{Persona, Severity},
    workflow::Workflow,
};

// Khatami et al. (SCAM 2024), KZ-20 "Specify package versions", and zizmor's
// `adhoc-packages`/`unpinned-tools` audits: https://docs.zizmor.sh/audits/
pub const UNPINNED_PACKAGE: Rule = Rule {
    id: "unpinned-package",
    summary: "Tools installed in run steps must name an exact version.",
    tag: "unpinned-package",
    severity: Severity::Warning,
    persona: Persona::Pedantic,
    equivalents: &[("zizmor", "adhoc-packages"), ("zizmor", "unpinned-tools")],
    configurable: false,
};

// Poutine `unverified_script_exec` and Semgrep `curl-eval`: a downloaded script runs
// before anything checks its integrity.
pub const REMOTE_SCRIPT: Rule = Rule {
    id: "unverified-remote-script",
    summary: "Downloaded scripts must not run before their integrity is checked.",
    tag: "unverified-remote-script",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[],
    configurable: false,
};

// zizmor `typosquat-uses`: an owner one edit away from a well-known publisher.
pub const TYPOSQUATTING: Rule = Rule {
    id: "typosquatted-action",
    summary: "Action owners one edit away from a well-known publisher may be typosquats.",
    tag: "typosquatting/impostor-commit",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "typosquat-uses")],
    configurable: false,
};

// zizmor `forbidden-uses`: an allowlist or denylist of `uses` repository patterns. It does
// nothing until configured. https://docs.zizmor.sh/audits/#forbidden-uses
pub const FORBIDDEN: Rule = Rule {
    id: "forbidden-action",
    summary: "`uses` must match the configured allowlist or denylist.",
    tag: "untrusted-3rd-party-action",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[("zizmor", "forbidden-uses")],
    configurable: true,
};

pub fn forbidden(workflow: &Workflow, options: Options) -> Result<Vec<Finding>, String> {
    let patterns = |key: &str| -> Option<Vec<&str>> {
        let list = options?.get(key)?.as_sequence()?;
        Some(list.iter().filter_map(Value::as_str).collect())
    };
    let (allow, deny) = (patterns("allow"), patterns("deny"));
    if allow.is_none() && deny.is_none() {
        return Ok(Vec::new());
    }
    let matches = |list: &Option<Vec<&str>>, uses: &str| {
        list.as_ref().is_some_and(|list| {
            list.iter()
                .any(|pattern| common::pattern_specificity(pattern, uses).is_some())
        })
    };
    let mut findings = Vec::new();
    for (route, location, uses) in common::all_uses(workflow) {
        // Repository refs only, like zizmor: local paths and `docker://` images are skipped.
        if common::parse_uses(uses).is_none() || uses.starts_with("docker://") {
            continue;
        }
        let forbidden = matches(&deny, uses) || (allow.is_some() && !matches(&allow, uses));
        if forbidden {
            findings.push(common::finding(
                workflow,
                &FORBIDDEN,
                route,
                location,
                format!("`{uses}` is not allowed by the forbidden-uses policy"),
            )?);
        }
    }
    Ok(findings)
}

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
                        "`{}` installs a package without an exact version",
                        command.join(" ")
                    ),
                )?);
            }
        }
    }
    Ok(findings)
}

/// Shell commands split on newlines and control operators, without `sudo` and env assignments.
/// A `${{ }}` expression stays inside one word, and quotes around a word are dropped.
fn commands(script: &str) -> impl Iterator<Item = Vec<&str>> {
    let bytes = script.as_bytes();
    let mut commands: Vec<Vec<&str>> = vec![Vec::new()];
    let mut start = None;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"${{") {
            start.get_or_insert(index);
            index = bytes[index..]
                .windows(2)
                .position(|pair| pair == b"}}")
                .map_or(bytes.len(), |end| index + end + 2);
            continue;
        }
        let byte = bytes[index];
        if matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b';' | b'|' | b'&') {
            if let Some(begin) = start.take() {
                commands.last_mut().unwrap().push(&script[begin..index]);
            }
            if matches!(byte, b'\n' | b';' | b'|' | b'&') {
                commands.push(Vec::new());
            }
        } else {
            start.get_or_insert(index);
        }
        index += 1;
    }
    if let Some(begin) = start {
        commands.last_mut().unwrap().push(&script[begin..]);
    }
    commands
        .into_iter()
        .map(|words| {
            words
                .into_iter()
                .map(|word| word.trim_matches(['"', '\'']))
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
    // Global flags may precede the subcommand, as in `npm -g install`.
    let words: Vec<&str> = words
        .iter()
        .take(1)
        .chain(
            words
                .iter()
                .skip(1)
                .skip_while(|word| word.starts_with('-')),
        )
        .copied()
        .collect();
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
        // Options that take a value: npm's config definitions (workspaces/config/lib/definitions/
        // definitions.js in npm/cli), and https://pnpm.io/pnpm-cli plus https://pnpm.io/cli/add,
        // where `-w` and `--workspace` take none.
        ["npm", "install" | "i" | "add", args @ ..] => arguments(
            args,
            &[
                "--prefix",
                "--registry",
                "-w",
                "--workspace",
                "--loglevel",
                "--omit",
                "--include",
                "--tag",
            ],
        )
        .any(|package| !has_npm_version(package) && !is_local_or_url(package)),
        ["pnpm", "install" | "i" | "add", args @ ..] => arguments(
            args,
            &["--registry", "--loglevel", "--filter", "-F", "--dir", "-C"],
        )
        .any(|package| !has_npm_version(package) && !is_local_or_url(package)),
        ["yarn", "global", "add", args @ ..] => arguments(args, &["--prefix", "--registry"])
            .any(|package| !has_npm_version(package) && !is_local_or_url(package)),
        // A module version is `v<semver>` or a commit hash; `latest` and branch names move.
        ["go", "install", args @ ..] => arguments(args, &["-C", "-o"]).any(|module| {
            !is_local_or_url(module)
                && module.rsplit_once('@').is_none_or(|(_, version)| {
                    !(version.starts_with('v')
                        && version[1..].starts_with(|c: char| c.is_ascii_digit())
                        || version.len() >= 7
                            && version.bytes().all(|byte| byte.is_ascii_hexdigit()))
                })
        }),
        // conda's match spec pins with `name=version` or `name==version`.
        ["conda" | "mamba" | "micromamba", "install", args @ ..] => arguments(
            args,
            &[
                "-c",
                "--channel",
                "-n",
                "--name",
                "-p",
                "--prefix",
                "--file",
                "-r",
                "--root-prefix",
            ],
        )
        .any(|package| !package.contains('=') || package.contains(['<', '>'])),
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

/// An exact `major.minor.patch` version after the name, with optional prerelease or build
/// metadata. Dist-tags (`latest`), wildcards (`*`, `1.x`), partial versions, and ranges move.
fn has_npm_version(package: &str) -> bool {
    let Some(index) = package.rfind('@').filter(|index| *index > 0) else {
        return false;
    };
    let version = package[index + 1..].trim_start_matches(['v', '=']);
    let core = version.split(['-', '+']).next().unwrap_or("");
    let numbers: Vec<&str> = core.split('.').collect();
    numbers.len() == 3
        && numbers
            .iter()
            .all(|number| !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()))
}

pub fn remote_script(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some(script) = step.run() else { continue };
            // A trailing `\` or `|` continues the command on the next line.
            let joined = without_comments(script)
                .replace("\\\n", " ")
                .replace("|\n", "| ");
            if joined.lines().any(pipes_download_to_interpreter) || runs_downloaded_file(&joined) {
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

/// `script` without shell comments: a `#` that starts a word outside quotes ends the line.
// ponytail: quotes are tracked per line, so a comment inside a multi-line string is dropped too.
fn without_comments(script: &str) -> String {
    script
        .lines()
        .map(|line| {
            let mut quote = None;
            let mut previous = ' ';
            for (index, character) in line.char_indices() {
                match (quote, character) {
                    (None, '"' | '\'') => quote = Some(character),
                    (Some(open), _) if open == character => quote = None,
                    (None, '#') if previous.is_whitespace() => return &line[..index],
                    _ => {}
                }
                previous = character;
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Interpreters, the short options that make them run code from an argument and leave stdin as
/// data (`bash -c`, `python3 -c`/`-m`, `perl -ne`, `node -e`/`-p`; `bash -s` still runs stdin),
/// and the short options whose value fills the rest of the word, as in `perl -I/usr/share/perl5`.
/// From bash(1), https://docs.python.org/3/using/cmdline.html, perlrun(1), ruby(1), and
/// https://nodejs.org/api/cli.html.
const INTERPRETERS: &[(&str, &[char], &[char])] = &[
    ("sh", &['c'], &['o', 'O']),
    ("bash", &['c'], &['o', 'O']),
    ("zsh", &['c'], &['o', 'O']),
    ("dash", &['c'], &['o', 'O']),
    ("ksh", &['c'], &['o', 'O']),
    ("python", &['c', 'm'], &['W', 'X']),
    ("python3", &['c', 'm'], &['W', 'X']),
    (
        "perl",
        &['e', 'E'],
        &['0', 'C', 'd', 'D', 'i', 'I', 'l', 'm', 'M', 'x', 'V'],
    ),
    (
        "ruby",
        &['e'],
        &['0', 'C', 'E', 'F', 'i', 'I', 'r', 'T', 'W', 'x'],
    ),
    ("node", &['e', 'p'], &['r', 'C']),
];

/// The interpreter a command runs and whether its options make it run code from an argument.
fn interpreter(command: &str) -> Option<(&'static str, bool, Vec<&str>)> {
    let mut words = command
        .split_whitespace()
        .map(|word| word.trim_matches(['"', '\'']))
        .skip_while(|word| *word == "sudo" || word.starts_with('-') || word.contains('='));
    let program = words.next()?;
    let program = program.rsplit('/').next().unwrap_or(program);
    let (name, code_options, valued) = INTERPRETERS.iter().find(|(name, ..)| *name == program)?;
    let mut arguments = Vec::new();
    let mut runs_argument = false;
    for word in words.by_ref() {
        if word == "--" || !word.starts_with('-') {
            arguments.push(word);
            break;
        }
        if matches!(word, "--eval" | "--print") {
            runs_argument = true;
        } else if !word.starts_with("--") {
            // Only the option's own letters: a valued option takes the rest of the word.
            for letter in word.chars().skip(1) {
                if code_options.contains(&letter) {
                    runs_argument = true;
                }
                if valued.contains(&letter) {
                    break;
                }
            }
        }
    }
    arguments.extend(words);
    Some((name, runs_argument, arguments))
}

/// Whether the command in `segment` is an interpreter that executes its stdin.
fn runs_stdin(segment: &str) -> bool {
    interpreter(segment).is_some_and(|(_, runs_argument, _)| !runs_argument)
}

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
        if downloaded && runs_stdin(segment) {
            return true;
        }
        downloaded = downloaded || downloads(segment);
    }
    false
}

/// Checksum and signature tools; running one after a download counts as its integrity check.
const VERIFIERS: &[&str] = &[
    "sha256sum",
    "sha512sum",
    "shasum",
    "gpg",
    "cosign",
    "minisign",
];

/// Short options of curl(1) and wget(1) whose value is the next word or the rest of the word.
const CURL_VALUED: &[char] = &[
    'A', 'b', 'c', 'C', 'd', 'D', 'e', 'E', 'F', 'H', 'K', 'm', 'P', 'Q', 'r', 't', 'T', 'u', 'U',
    'w', 'x', 'X', 'y', 'Y', 'z',
];
const WGET_VALUED: &[char] = &[
    'a', 'A', 'B', 'D', 'e', 'i', 'I', 'l', 'o', 'P', 'Q', 'R', 't', 'T', 'U', 'w', 'X',
];

/// File names a `curl` or `wget` command saves: curl's `-o file` and `-O` (the URL's last
/// segment), and wget's `-O file` or, without it, the URL's last segment. `-O -` is stdout.
fn saved_files(words: &[&str]) -> Vec<String> {
    let (output, remote, valued) = match words.first() {
        Some(&"curl") => ('o', Some('O'), CURL_VALUED),
        Some(&"wget") => ('O', None, WGET_VALUED),
        _ => return Vec::new(),
    };
    let url_name = || {
        words
            .iter()
            .find(|word| word.contains("://"))
            .and_then(|url| url.split(['?', '#']).next())
            .and_then(|url| url.rsplit('/').next())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    };
    let mut files = Vec::new();
    let mut named = false;
    let mut rest = words[1..].iter();
    while let Some(word) = rest.next() {
        let value = match *word {
            "--output" | "--output-document" => rest.next().copied(),
            "--remote-name" if remote.is_some() => Some(""),
            _ => match word.strip_prefix("--output-document=") {
                Some(file) => Some(file),
                None if word.starts_with('-') && !word.starts_with("--") => {
                    let mut found = None;
                    for (index, letter) in word.char_indices().skip(1) {
                        if Some(letter) == remote {
                            found = Some("");
                        } else if letter == output {
                            let attached = &word[index + 1..];
                            found = match attached.is_empty() {
                                true => rest.next().copied(),
                                false => Some(attached),
                            };
                            break;
                        } else if valued.contains(&letter) {
                            break;
                        }
                    }
                    found
                }
                None => None,
            },
        };
        match value {
            None => continue,
            Some("-") => {}
            // curl's `-O` saves under the URL's last segment.
            Some("") => files.extend(url_name()),
            Some(file) => files.push(file.to_owned()),
        }
        named = true;
    }
    if !named && words[0] == "wget" {
        files.extend(url_name());
    }
    files
}

/// Whether a script saves a download to a file and then runs that file before any checksum or
/// signature tool runs.
fn runs_downloaded_file(script: &str) -> bool {
    let name = |path: &str| path.rsplit('/').next().unwrap_or(path).to_owned();
    let mut downloaded: Vec<String> = Vec::new();
    for command in script.split(['\n', ';', '&', '|']) {
        let words: Vec<&str> = command
            .split_whitespace()
            .map(|word| word.trim_matches(['"', '\'']))
            .skip_while(|word| *word == "sudo" || word.starts_with('-') || word.contains('='))
            .collect();
        let Some(program) = words.first() else {
            continue;
        };
        if VERIFIERS.contains(&name(program).as_str()) {
            downloaded.clear();
            continue;
        }
        downloaded.extend(saved_files(&words).iter().map(|file| name(file)));
        let runs = match interpreter(command) {
            Some((_, false, arguments)) => arguments.first().map(|script| name(script)),
            Some((_, true, _)) => None,
            None if matches!(*program, "source" | ".") => words.get(1).map(|file| name(file)),
            None if program.contains('/') => Some(name(program)),
            None => None,
        };
        if runs.is_some_and(|file| downloaded.contains(&file)) {
            return true;
        }
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
