use yaml_serde::Value;

use super::{
    Finding, Options, Rule, common,
    shell::{self, Arg, Program, Spec},
};
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
    relational: false,
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
    relational: false,
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
    relational: false,
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
    relational: false,
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
            let commands = shell::commands(script);
            let unpinned = commands
                .iter()
                .filter_map(|command| shell::program(&command.words))
                .find(unpinned_install);
            if let Some(program) = unpinned {
                findings.push(common::finding(
                    workflow,
                    &UNPINNED_PACKAGE,
                    step.key("run"),
                    step.location.clone(),
                    format!(
                        "`{} {}` installs a package without an exact version",
                        program.path,
                        program.args.join(" ")
                    ),
                )?);
            }
        }
    }
    Ok(findings)
}

/// Options that take a value. npm: its config definitions (workspaces/config/lib/definitions/
/// definitions.js in npm/cli); pnpm: https://pnpm.io/pnpm-cli and https://pnpm.io/cli/add,
/// where `-w` and `--workspace` take none.
const PIP: Spec = Spec {
    short: &['r', 'c', 'e', 'i', 'f', 't'],
    long: &[
        "requirement",
        "constraint",
        "editable",
        "index-url",
        "extra-index-url",
        "find-links",
        "target",
    ],
};
const NPM: Spec = Spec {
    short: &['w'],
    long: &[
        "prefix",
        "registry",
        "workspace",
        "loglevel",
        "omit",
        "include",
        "tag",
    ],
};
const PNPM: Spec = Spec {
    short: &['F', 'C'],
    long: &["registry", "loglevel", "filter", "dir"],
};
const YARN: Spec = Spec {
    short: &[],
    long: &["prefix", "registry"],
};
const GO: Spec = Spec {
    short: &['C', 'o'],
    long: &[],
};
const CONDA: Spec = Spec {
    short: &['c', 'n', 'p', 'r'],
    long: &["channel", "name", "prefix", "file", "root-prefix"],
};
const CARGO: Spec = Spec {
    short: &['F', 'j'],
    long: &[
        "root", "features", "target", "jobs", "profile", "bin", "example", "registry", "index",
        "branch", "tag", "rev",
    ],
};
const GEM: Spec = Spec {
    short: &['s', 'i'],
    long: &["source", "install-dir"],
};

fn unpinned_install(program: &Program) -> bool {
    let words: Vec<&str> = std::iter::once(program.name)
        .chain(program.args.iter().copied())
        .collect();
    let words: Vec<&str> = match words.as_slice() {
        ["python" | "python3", "-m", rest @ ..] => rest.to_vec(),
        ["uv", "pip", rest @ ..] => [&["pip"][..], rest].concat(),
        ["uv", "tool", rest @ ..] => [&["pipx"][..], rest].concat(),
        _ => words,
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
    let npm_unpinned = |package: &str| !has_npm_version(package) && !is_local_or_url(package);
    match words.as_slice() {
        ["pip" | "pip3" | "pipx", "install", args @ ..] => {
            packages(args, &PIP).any(|package| !package.contains("==") && !is_local_or_url(package))
        }
        ["npm", "install" | "i" | "add", args @ ..] => packages(args, &NPM).any(npm_unpinned),
        ["pnpm", "install" | "i" | "add", args @ ..] => packages(args, &PNPM).any(npm_unpinned),
        ["yarn", "global", "add", args @ ..] => packages(args, &YARN).any(npm_unpinned),
        // A module version is `v<semver>` or a commit hash; `latest` and branch names move.
        ["go", "install", args @ ..] => packages(args, &GO).any(|module| {
            !is_local_or_url(module)
                && module.rsplit_once('@').is_none_or(|(_, version)| {
                    !(version.starts_with('v')
                        && version[1..].starts_with(|c: char| c.is_ascii_digit())
                        || version.len() >= 7
                            && version.bytes().all(|byte| byte.is_ascii_hexdigit()))
                })
        }),
        // conda's match spec pins with `name=version` or `name==version`.
        ["conda" | "mamba" | "micromamba", "install", args @ ..] => packages(args, &CONDA)
            .any(|package| !package.contains('=') || package.contains(['<', '>'])),
        ["cargo", "install", args @ ..] => {
            let pinned_by_flag = args
                .iter()
                .any(|arg| matches!(*arg, "--version" | "--vers" | "--path" | "--git"));
            !pinned_by_flag && packages(args, &CARGO).any(|name| !name.contains('@'))
        }
        ["gem", "install", args @ ..] => {
            !args.iter().any(|arg| matches!(*arg, "-v" | "--version"))
                && packages(args, &GEM).any(|name| !name.contains(':'))
        }
        _ => false,
    }
}

/// Operands that name a package, skipping shell variables, expressions, and redirects.
fn packages<'a>(args: &'a [&'a str], spec: &'a Spec) -> impl Iterator<Item = &'a str> + 'a {
    shell::args(args, spec)
        .filter_map(|arg| match arg {
            Arg::Operand(operand) => Some(operand),
            _ => None,
        })
        .filter(|arg| !arg.contains("${{") && !arg.starts_with('$') && !arg.starts_with('>'))
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
            if pipes_download_to_interpreter(script) || runs_downloaded_file(script) {
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
/// and the short options that take a value, as in `perl -I/usr/share/perl5`.
/// From bash(1), https://docs.python.org/3/using/cmdline.html, perlrun(1), ruby(1), and
/// https://nodejs.org/api/cli.html.
const INTERPRETERS: &[(&str, &[char], Spec)] = &[
    ("sh", &['c'], SHELL),
    ("bash", &['c'], SHELL),
    ("zsh", &['c'], SHELL),
    ("dash", &['c'], SHELL),
    ("ksh", &['c'], SHELL),
    ("python", &['c', 'm'], PYTHON),
    ("python3", &['c', 'm'], PYTHON),
    (
        "perl",
        &['e', 'E'],
        Spec {
            short: &['0', 'C', 'd', 'D', 'i', 'I', 'l', 'm', 'M', 'x', 'V'],
            long: &[],
        },
    ),
    (
        "ruby",
        &['e'],
        Spec {
            short: &['0', 'C', 'E', 'F', 'i', 'I', 'r', 'T', 'W', 'x'],
            long: &[],
        },
    ),
    (
        "node",
        &['e', 'p'],
        Spec {
            short: &['r', 'C'],
            long: &[],
        },
    ),
];
const SHELL: Spec = Spec {
    short: &['o', 'O'],
    long: &[],
};
const PYTHON: Spec = Spec {
    short: &['W', 'X'],
    long: &[],
};

/// When `program` is an interpreter: whether its options make it run code from an argument, and
/// the words from its first operand (the script) on.
fn interpreter<'a, 'w>(program: &Program<'a, 'w>) -> Option<(bool, &'w [&'a str])> {
    let (_, code, spec) = INTERPRETERS
        .iter()
        .find(|(name, ..)| *name == program.name)?;
    let (options, operands) = shell::leading_options(program.args, spec);
    let runs_argument = options.iter().any(|option| match option {
        Arg::Short(letter, _) => code.contains(letter),
        Arg::Long(name, _) => matches!(*name, "eval" | "print"),
        Arg::Operand(_) => false,
    });
    Some((runs_argument, operands))
}

/// Whether a download reaches an interpreter that runs its stdin, through a pipe or a process
/// or command substitution.
fn pipes_download_to_interpreter(script: &str) -> bool {
    let substituted = without_comments(script)
        .replace("\\\n", " ")
        .lines()
        .any(|line| {
            line.contains("<(curl")
                || line.contains("<(wget")
                || (line.contains("$(curl") || line.contains("$(wget"))
                    && ["sh -c", "bash -c", "eval"]
                        .iter()
                        .any(|runner| line.contains(runner))
        });
    let mut downloaded = false;
    substituted
        || shell::commands(script).iter().any(|command| {
            downloaded &= command.piped;
            let runs_stdin = shell::program(&command.words)
                .and_then(|program| interpreter(&program))
                .is_some_and(|(runs_argument, _)| !runs_argument);
            let pipes = downloaded && runs_stdin;
            downloaded |= command
                .words
                .iter()
                .any(|word| word.contains("curl") || word.contains("wget"));
            pipes
        })
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

/// Short options of curl(1) and wget(1) that take a value, plus the output options.
const CURL: Spec = Spec {
    short: &[
        'A', 'b', 'c', 'C', 'd', 'D', 'e', 'E', 'F', 'H', 'K', 'm', 'o', 'P', 'Q', 'r', 't', 'T',
        'u', 'U', 'w', 'x', 'X', 'y', 'Y', 'z',
    ],
    long: &["output"],
};
const WGET: Spec = Spec {
    short: &[
        'a', 'A', 'B', 'D', 'e', 'i', 'I', 'l', 'o', 'O', 'P', 'Q', 'R', 't', 'T', 'U', 'w', 'X',
    ],
    long: &["output-document"],
};

/// File names a `curl` or `wget` command saves: curl's `-o file` and `-O` (the URL's last
/// segment), and wget's `-O file` or, without it, the URL's last segment. `-O -` is stdout.
fn saved_files(program: &Program) -> Vec<String> {
    let (output, remote, spec) = match program.name {
        "curl" => ('o', Some('O'), &CURL),
        "wget" => ('O', None, &WGET),
        _ => return Vec::new(),
    };
    let url_name = || {
        program
            .args
            .iter()
            .find(|word| word.contains("://"))
            .and_then(|url| url.split(['?', '#']).next())
            .and_then(|url| url.rsplit('/').next())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    };
    let mut files = Vec::new();
    let mut named = false;
    for arg in shell::args(program.args, spec) {
        let value = match arg {
            Arg::Short(letter, value) if letter == output => value,
            Arg::Long("output" | "output-document", value) => value,
            // curl's `-O` saves under the URL's last segment.
            Arg::Short(letter, _) if Some(letter) == remote => Some(""),
            Arg::Long("remote-name", _) if remote.is_some() => Some(""),
            _ => None,
        };
        match value {
            None => continue,
            Some("-") => {}
            Some("") => files.extend(url_name()),
            Some(file) => files.push(file.to_owned()),
        }
        named = true;
    }
    if !named && program.name == "wget" {
        files.extend(url_name());
    }
    files
}

/// Whether a script saves a download to a file and then runs that file before any checksum or
/// signature tool runs.
fn runs_downloaded_file(script: &str) -> bool {
    let mut downloaded: Vec<String> = Vec::new();
    for command in shell::commands(script) {
        let Some(program) = shell::program(&command.words) else {
            continue;
        };
        if VERIFIERS.contains(&program.name) {
            downloaded.clear();
            continue;
        }
        downloaded.extend(
            saved_files(&program)
                .iter()
                .map(|file| shell::basename(file).to_owned()),
        );
        let runs = match interpreter(&program) {
            Some((false, operands)) => operands.first().copied(),
            Some((true, _)) => None,
            None if matches!(program.name, "source" | ".") => program.args.first().copied(),
            None if program.path.contains('/') => Some(program.path),
            None => None,
        };
        if runs.is_some_and(|file| {
            downloaded
                .iter()
                .any(|saved| saved == shell::basename(file))
        }) {
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
