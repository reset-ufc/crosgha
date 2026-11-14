//! Workflow commands, runner images, action versions, and action inputs that GitHub or the action
//! deprecated or removed.

use yaml_serde::Value;
use yamlpatch::{Op, Patch};
use yamlpath::Route;

use super::{
    ChangeKind, Edit, Finding, LineRewrite, ProposedFix, Rule, common, repository, runner, shell,
};
use crate::policy::{Persona, Severity};
use crate::workflow::Workflow;

// GitHub disabled `set-env`/`add-path` and deprecated `set-output`/`save-state`:
// https://github.blog/changelog/2020-10-01-github-actions-deprecating-set-env-and-add-path-commands/
// https://github.blog/changelog/2022-10-11-github-actions-deprecating-save-state-and-set-output-commands/
// GitHub removed the `ACTIONS_ALLOW_USE_UNSECURE_NODE_VERSION` opt-out along with Node 20:
// https://github.blog/changelog/2026-09-23-node-20-is-no-longer-available-in-github-actions/
// Hosted runner images, action versions, and action inputs that GitHub or the action removed, so the
// job no longer runs. Sources are next to each list below.
pub const REMOVED: Rule = Rule {
    id: "removed-feature",
    summary: "Retired runner images, shut-down action versions, and removed action inputs must not be used.",
    tag: "deprecated-command/feature",
    severity: Severity::Error,
    persona: Persona::Regular,
    equivalents: &[
        ("actionlint", "runner-label"),
        ("actionlint", "action"),
        ("actionlint", "deprecated-inputs"),
    ],
    configurable: false,
    relational: false,
};

pub const DEPRECATED: Rule = Rule {
    id: "deprecated-command",
    summary: "Deprecated workflow commands and removed opt-out variables must not be used.",
    tag: "deprecated-command/feature",
    severity: Severity::Warning,
    persona: Persona::Regular,
    equivalents: &[
        ("zizmor", "insecure-commands"),
        ("actionlint", "deprecated-commands"),
    ],
    configurable: false,
    relational: false,
};

const DEPRECATED_COMMANDS: &[&str] = &["::set-output", "::save-state", "::set-env", "::add-path"];

const OPT_OUTS: &[(&str, &str)] = &[
    (
        "ACTIONS_ALLOW_UNSECURE_COMMANDS",
        "re-enables the disabled `set-env` and `add-path` commands",
    ),
    (
        "ACTIONS_ALLOW_USE_UNSECURE_NODE_VERSION",
        "the Node 20 opt-out was removed; update the action to one that runs on Node 24",
    ),
];

pub fn deprecated(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    let mut scopes = vec![(Route::default(), "workflow".to_owned(), &workflow.value)];
    for job in common::jobs(workflow) {
        scopes.push((job.route.clone(), job.location(), job.value));
        for step in common::steps(&job) {
            if let Some(script) = step.run()
                && let Some(command) = deprecated_command(script)
            {
                let mut finding = common::finding(
                    workflow,
                    &DEPRECATED,
                    step.key("run"),
                    step.location.clone(),
                    format!("`{command}` is a deprecated workflow command"),
                )?;
                finding.fix = environment_files(workflow, &job, &step, script);
                findings.push(finding);
            }
            scopes.push((step.route.clone(), step.location.clone(), step.value));
        }
    }
    for (route, location, scope) in scopes {
        for (variable, message) in OPT_OUTS {
            // The runner reads it with .NET `bool.TryParse`, which accepts `True` in any case
            // (`ActionCommandManager.cs` in actions/runner at
            // d7bc179baf11a02110b46cfbbc4040f74ac3f60a); an expression may evaluate to it.
            let value = scope.get("env").and_then(|env| env.get(variable));
            if value.is_some_and(|value| {
                common::input_bool(value) == Some(true)
                    || value.as_str().is_some_and(|text| text.contains("${{"))
            }) {
                findings.push(common::finding(
                    workflow,
                    &DEPRECATED,
                    route.with_key("env").with_key(*variable),
                    location.clone(),
                    *message,
                )?);
            }
        }
    }
    Ok(findings)
}

/// The first deprecated command that `script` runs.
fn deprecated_command(script: &str) -> Option<&'static str> {
    let commands = shell::commands(script);
    DEPRECATED_COMMANDS.iter().copied().find(|command| {
        commands
            .iter()
            .flat_map(|run| &run.words)
            .any(|word| word.contains(command))
    })
}

/// The environment file that replaces each command that takes a `name`:
/// https://github.blog/changelog/2022-10-11-github-actions-deprecating-save-state-and-set-output-commands/
/// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#setting-an-environment-variable
const NAMED_FILES: &[(&str, &str)] = &[
    ("set-output", "GITHUB_OUTPUT"),
    ("save-state", "GITHUB_STATE"),
    ("set-env", "GITHUB_ENV"),
];

/// Script text that leaves a step unfixed. A heredoc or here-string (`<<`) may hold a command as
/// data, and `$'...'` quoting hides where quotes end. The runner decoded `%25`, `%0D`, and `%0A`
/// in a command's value, but an environment file stores them as written
/// (`src/Runner.Common/ActionCommand.cs` in actions/runner at
/// 8cfa1049f86781d141056082d73802d56cd2b433), so a script that escapes values that way would
/// change what it writes.
const UNFIXABLE: &[&str] = &["<<", "$'", "%25", "%0D", "%0A"];

/// Rewrites each deprecated command of a bash or sh step into the environment file write that
/// replaces it, or `None` when one of them is not a whole `echo` line with a known rewrite.
fn environment_files(
    workflow: &Workflow,
    job: &common::Job,
    step: &common::Step,
    script: &str,
) -> Option<ProposedFix> {
    if !shell::runs_posix(workflow, job, step) || UNFIXABLE.iter().any(|text| script.contains(text))
    {
        return None;
    }
    // Only a literal block scalar or a one-line plain scalar holds the script as written.
    let route = step.key("run");
    let node = workflow.document.query_exact(&route).ok()??;
    let source = workflow.document.extract(&node);
    if !source.starts_with('|') && source != script {
        return None;
    }
    let mut rewrites = Vec::new();
    for (line, top_level) in lines(script) {
        if deprecated_command(line).is_none() {
            continue;
        }
        let from = line.trim();
        let to = top_level.then(|| rewrite(from))??;
        rewrites.push(LineRewrite {
            from: from.to_owned(),
            to,
        });
    }
    if rewrites.is_empty() {
        return None;
    }
    Some(ProposedFix {
        edit: Edit::Lines(rewrites),
        changed_route: route,
        change: ChangeKind::Replace,
    })
}

/// Each line of `script`, and whether it starts a new command: not inside a quoted string and not
/// continued from the line before by a trailing `\`.
fn lines(script: &str) -> Vec<(&str, bool)> {
    let mut found = Vec::new();
    let mut quote = None;
    let mut continued = false;
    for line in script.split('\n') {
        found.push((line, quote.is_none() && !continued));
        continued = false;
        let bytes = line.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            match (quote, byte) {
                (Some(b'\''), b'\'') | (Some(b'"'), b'"') => quote = None,
                (Some(b'"') | None, b'\\') => {
                    continued = index + 1 == bytes.len();
                    index += 1;
                }
                (None, b'\'' | b'"') => quote = Some(byte),
                (None, b'#') if index == 0 || bytes[index - 1].is_ascii_whitespace() => break,
                _ => {}
            }
            index += 1;
        }
    }
    found
}

/// The environment file write that replaces `line` when it is exactly
/// `echo <quote>::<command> name=<name>::<value><quote>` or `echo <quote>::add-path::<path><quote>`.
/// The write keeps `echo` and the quoting, so the shell expands the value as before.
fn rewrite(line: &str) -> Option<String> {
    let argument = line.strip_prefix("echo ")?;
    let quote = match argument.as_bytes().first() {
        Some(b'"') => "\"",
        Some(b'\'') => "'",
        _ => "",
    };
    let text = argument
        .strip_prefix(quote)?
        .strip_suffix(quote)
        .filter(|text| quote.is_empty() || !text.contains(quote))?;
    let (head, value) = text.strip_prefix("::")?.split_once("::")?;
    let quoted_safely = match quote {
        "\"" => !value.ends_with('\\'),
        "'" => true,
        _ => unquoted_safe(value),
    };
    if !quoted_safely {
        return None;
    }
    let (file, written) = match head.split_once(' ') {
        None if head == "add-path" && !value.is_empty() => ("GITHUB_PATH", value.to_owned()),
        Some((command, property)) => {
            let (_, file) = NAMED_FILES.iter().find(|(name, _)| *name == command)?;
            let name = property
                .strip_prefix("name=")
                .filter(|name| !name.is_empty() && name.bytes().all(is_name_byte))?;
            (*file, format!("{name}={value}"))
        }
        None => return None,
    };
    Some(format!("echo {quote}{written}{quote} >> \"${{{file}}}\""))
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

/// Whether an unquoted value reads the same after its word moves: only plain characters and
/// `$NAME` or `${NAME}` expansions, nothing that globs, expands braces, or ends the command.
fn unquoted_safe(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let rest = &bytes[index..];
        let length = match rest {
            [b'$', b'{', ..] => match rest.iter().position(|byte| *byte == b'}') {
                Some(close)
                    if close > 2 && rest[2..close].iter().all(|byte| is_name_byte(*byte)) =>
                {
                    close + 1
                }
                _ => return false,
            },
            [b'$', ..] => 1,
            [byte, ..] if is_name_byte(*byte) || b"./:@+=, ".contains(byte) => 1,
            _ => return false,
        };
        index += length;
    }
    true
}

/// Hosted images and the date GitHub fully retired them:
/// https://github.com/actions/runner-images/issues/6002 (ubuntu-18.04)
/// https://github.blog/changelog/2025-01-15-github-actions-ubuntu-20-runner-image-brownout-dates-and-other-breaking-changes/
/// https://github.blog/changelog/2025-04-15-upcoming-breaking-changes-and-releases-for-github-actions/ (windows-2019)
/// https://github.blog/changelog/2025-07-11-upcoming-changes-to-macos-hosted-runners-macos-latest-migration-and-xcode-support-policy-updates/
// ponytail: a fixed list with no runtime lookup; add each image when GitHub retires it.
const RETIRED_RUNNERS: &[(&str, &str)] = &[
    // https://github.com/actions/runner-images/issues/3287
    ("ubuntu-16.04", "2021-09-20"),
    // https://github.com/actions/runner-images/issues/4312
    ("windows-2016", "2022-03-15"),
    // https://github.com/actions/runner-images/issues/5583
    ("macos-10.15", "2022-12-01"),
    // https://github.blog/changelog/2024-05-20-actions-upcoming-changes-to-github-hosted-macos-runners/
    ("macos-11", "2024-06-28"),
    // https://github.com/actions/runner-images/issues/10721
    ("macos-12", "2024-12-03"),
    ("ubuntu-18.04", "2023-04-03"),
    ("ubuntu-20.04", "2025-04-15"),
    ("windows-2019", "2025-06-30"),
    ("macos-13", "2025-12-04"),
];

/// Why an action version no longer runs, from its `major.minor` numbers.
fn shut_down(action: &str, version: &[u64]) -> Option<&'static str> {
    match (action, version) {
        // https://github.blog/changelog/2024-04-16-deprecation-notice-v3-of-the-artifact-actions/
        ("actions/upload-artifact" | "actions/download-artifact", [major, ..]) if *major <= 3 => {
            Some("artifact actions v3 and older stopped working on 2025-01-30; use v4 or later")
        }
        // https://github.com/actions/cache/blob/main/README.md: "upgrade to the recommended
        // versions `v4.2.0` or `v3.4.0`", else "workflow runs ... will fail".
        ("actions/cache", [major, ..]) if *major <= 2 => Some(
            "actions/cache below v3.4.0 fails since the cache service moved; use v4.2.0 or later",
        ),
        ("actions/cache", [3, minor, ..]) if *minor < 4 => Some(
            "actions/cache below v3.4.0 fails since the cache service moved; use v4.2.0 or later",
        ),
        ("actions/cache", [4, minor, ..]) if *minor < 2 => Some(
            "actions/cache below v4.2.0 fails since the cache service moved; use v4.2.0 or later",
        ),
        _ => None,
    }
}

/// setup-java v6 removed the AdoptOpenJDK distributions: "Use `temurin` instead of `adopt` or
/// `adopt-hotspot`, and `semeru` instead of `adopt-openj9`."
/// https://github.com/actions/setup-java/blob/main/README.md
const ADOPT_SUCCESSORS: &[(&str, &str)] = &[
    ("adopt", "temurin"),
    ("adopt-hotspot", "temurin"),
    ("adopt-openj9", "semeru"),
];

/// github.com facts: retired hosted-runner images and action versions its services no longer
/// serve. Registered in `HOSTED`, so another `--gh-hostname` skips it.
pub fn removed_hosted(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        // Labels of a runner group or of a self-hosted runner name a machine, not a hosted image.
        let grouped = job
            .get("runs-on")
            .is_some_and(|runs_on| runs_on.get("group").is_some());
        let labels = runner::job_labels(&job);
        let retired = if grouped || labels.contains(&"self-hosted") {
            None
        } else {
            labels
                .iter()
                .find_map(|label| RETIRED_RUNNERS.iter().find(|(name, _)| name == label))
        };
        if let Some((label, date)) = retired {
            findings.push(common::finding(
                workflow,
                &REMOVED,
                job.route.with_key("runs-on"),
                job.location(),
                format!("the `{label}` hosted runner image was retired on {date}"),
            )?);
        }
        for step in common::steps(&job) {
            let Some((name, version)) = common::versioned_action(workflow, &step) else {
                continue;
            };
            if let Some(reason) = shut_down(&name, &version) {
                findings.push(common::finding(
                    workflow,
                    &REMOVED,
                    step.key("uses"),
                    step.location.clone(),
                    reason,
                )?);
            }
        }
    }
    Ok(findings)
}

/// Majors of GitHub's own JavaScript actions whose latest release declares a retired Node.js
/// runtime (`repository::RUNTIME`), read from `runs.using` in each major's newest `vX.Y.Z` tag on
/// 2026-09-27. A major is the unit because `@vN` follows its newest release.
// ponytail: `actions/*` only and major granularity, so a pin to an older minor of a node24 major
// (setup-go v6.0.0) is missed; per-release data or more owners would close both gaps.
const RETIRED_RUNTIME_MAJORS: &[(&str, &[(u64, &str)])] = &[
    ("actions/add-to-project", &[(0, "node20"), (1, "node20")]),
    (
        "actions/cache",
        &[
            (0, "node12"),
            (1, "node12"),
            (2, "node12"),
            (3, "node16"),
            (4, "node20"),
        ],
    ),
    (
        "actions/checkout",
        &[(2, "node12"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/configure-pages",
        &[
            (0, "node16"),
            (1, "node16"),
            (2, "node16"),
            (3, "node16"),
            (4, "node20"),
            (5, "node20"),
        ],
    ),
    (
        "actions/create-github-app-token",
        &[(1, "node20"), (2, "node20")],
    ),
    ("actions/create-release", &[(1, "node12")]),
    (
        "actions/dependency-review-action",
        &[(1, "node16"), (2, "node16"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/deploy-pages",
        &[(1, "node16"), (2, "node16"), (3, "node20"), (4, "node20")],
    ),
    (
        "actions/download-artifact",
        &[(2, "node12"), (4, "node20"), (5, "node20"), (6, "node20")],
    ),
    (
        "actions/github-script",
        &[
            (1, "node12"),
            (2, "node12"),
            (3, "node12"),
            (4, "node12"),
            (5, "node12"),
            (6, "node16"),
            (7, "node20"),
        ],
    ),
    (
        "actions/labeler",
        &[(2, "node12"), (3, "node12"), (4, "node16"), (5, "node20")],
    ),
    (
        "actions/setup-dotnet",
        &[(1, "node12"), (2, "node16"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/setup-go",
        &[
            (1, "node12"),
            (2, "node12"),
            (3, "node16"),
            (4, "node16"),
            (5, "node20"),
        ],
    ),
    (
        "actions/setup-java",
        &[(1, "node12"), (2, "node12"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/setup-node",
        &[(1, "node12"), (2, "node12"), (3, "node16"), (4, "node20")],
    ),
    (
        "actions/setup-python",
        &[
            (1, "node12"),
            (2, "node12"),
            (3, "node16"),
            (4, "node16"),
            (5, "node20"),
        ],
    ),
    ("actions/setup-ruby", &[(1, "node12")]),
    (
        "actions/stale",
        &[
            (1, "node12"),
            (2, "node12"),
            (3, "node12"),
            (4, "node12"),
            (5, "node16"),
            (6, "node16"),
            (7, "node16"),
            (8, "node16"),
            (9, "node20"),
        ],
    ),
    (
        "actions/upload-artifact",
        &[(2, "node12"), (4, "node20"), (5, "node20")],
    ),
    ("actions/upload-release-asset", &[(1, "node12")]),
];

/// Steps that use a version of an `actions/*` action built for a retired Node.js runtime.
pub fn retired_runtime(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some((name, version)) = common::versioned_action(workflow, &step) else {
                continue;
            };
            let runtime = RETIRED_RUNTIME_MAJORS
                .iter()
                .find(|(action, _)| *action == name)
                .and_then(|(_, majors)| {
                    majors
                        .iter()
                        .find(|(major, _)| Some(major) == version.first())
                });
            if let Some((major, runtime)) = runtime {
                findings.push(common::finding(
                    workflow,
                    &repository::RUNTIME,
                    step.key("uses"),
                    step.location.clone(),
                    format!(
                        "{name} v{major} declares `{runtime}`, which GitHub retired; runners force it onto Node 24, so use a release built for `node24`"
                    ),
                )?);
            }
        }
    }
    Ok(findings)
}

/// Inputs an action's newer major removed, whatever host runs it.
pub fn removed(workflow: &Workflow) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    for job in common::jobs(workflow) {
        for step in common::steps(&job) {
            let Some((name, version)) = common::versioned_action(workflow, &step) else {
                continue;
            };
            let successor = step
                .value
                .get("with")
                .and_then(|with| with.get("distribution"))
                .and_then(Value::as_str)
                .and_then(|distribution| {
                    ADOPT_SUCCESSORS
                        .iter()
                        .find(|(adopt, _)| *adopt == distribution)
                });
            if name == "actions/setup-java"
                && version.first().is_some_and(|major| *major >= 6)
                && let Some((adopt, successor)) = successor
            {
                let input = step.key("with").with_key("distribution");
                let mut finding = common::finding(
                    workflow,
                    &REMOVED,
                    step.key("uses"),
                    step.location.clone(),
                    format!("setup-java v6 removed the `{adopt}` distribution; use `{successor}`"),
                )?;
                finding.fix = Some(ProposedFix {
                    edit: Edit::Patch(Patch {
                        route: input.clone(),
                        operation: Op::Replace(Value::String((*successor).to_owned())),
                    }),
                    changed_route: input,
                    change: ChangeKind::Replace,
                });
                findings.push(finding);
            }
        }
    }
    Ok(findings)
}
