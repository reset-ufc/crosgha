use std::{
    collections::HashSet,
    fmt::Display,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{ColorChoice, Parser, Subcommand};

use crate::{
    config, engine, filesystem,
    github::GitHub,
    hook, migrate, policy,
    report::{self, FileReport, RunError, Unwritten},
    rules::{self, Located, Place},
};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Detect and fix GitHub Actions workflow smells.",
    args_conflicts_with_subcommands = true
)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Workflow files, workflow directories, or repository roots to scan.
    #[arg(value_name = "PATH", default_value = ".github/workflows")]
    paths: Vec<PathBuf>,

    #[command(flatten)]
    modes: Modes,

    /// Sensitivity: regular (default), pedantic (adds code smells), or auditor (everything).
    #[arg(long, value_enum, default_value_t = policy::Persona::Regular)]
    persona: policy::Persona,

    /// zizmor's alias for `--persona pedantic`: https://docs.zizmor.sh/usage/#using-personas
    #[arg(long, hide = true, conflicts_with = "persona")]
    pedantic: bool,

    /// Never contact the GitHub API, even when a token is available.
    #[arg(long)]
    offline: bool,

    /// GitHub token for online rules; also read from GH_TOKEN or GITHUB_TOKEN.
    #[arg(long, env = "GH_TOKEN", hide_env_values = true)]
    gh_token: Option<String>,

    /// GitHub host for online rules, such as a GitHub Enterprise Server domain.
    #[arg(long, env = "GH_HOST", default_value = "github.com")]
    gh_hostname: String,

    /// Skip online rules even when a token is available.
    #[arg(long)]
    no_online_audits: bool,

    /// Config file for every input; also read from CROSGHA_CONFIG or ZIZMOR_CONFIG.
    // The variables are read in `load_configs`, where an empty one counts as unset.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Do not discover or load crosgha.yml or zizmor.yml, even one named by `--config`.
    // No clap conflict, so the config variables also yield to `--no-config`.
    #[arg(long)]
    no_config: bool,

    /// Report findings that inline comments or the config ignore.
    #[arg(long)]
    no_ignores: bool,

    /// Hide findings below this zizmor severity: informational and low map to note,
    /// medium to warning, high to error.
    #[arg(long, value_parser = ["unknown", "informational", "low", "medium", "high"])]
    min_severity: Option<String>,

    /// Accepted for zizmor compatibility; crosgha findings carry no confidence level.
    #[arg(long, value_parser = ["unknown", "low", "medium", "high"])]
    min_confidence: Option<String>,

    /// `owner/name` whose Actions settings online rules check.
    #[arg(long, env = "GITHUB_REPOSITORY")]
    repository: Option<String>,

    /// Also write a SARIF report to this file, whatever `--format` prints.
    #[arg(long, value_name = "FILE")]
    sarif_file: Option<PathBuf>,

    /// Report format: text (also `plain`), json, sarif, github (workflow annotations), or agent
    /// (brief lines for AI agents). Defaults to github when GITHUB_ACTIONS=true, text otherwise.
    #[arg(short, long, value_enum)]
    format: Option<report::Format>,

    /// When to color text output: auto (a terminal without NO_COLOR), always, or never.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,

    /// Minimum severity that makes a scan fail (any, note, warning, error, or none).
    #[arg(long, value_enum, default_value_t = policy::FailOn::Any)]
    fail_on: policy::FailOn,
}

/// Flags that make a run do something other than a read-only scan, which the pre-push hook must
/// not store.
#[derive(Debug, clap::Args)]
struct Modes {
    /// Apply verified fixes to workflow files.
    #[arg(long)]
    fix: bool,

    /// Print every rule with its taxonomy tag, severity, persona, and scope, then exit;
    /// `--format json` adds each rule's summary, equivalents, and whether it reads a config.
    #[arg(long)]
    list_rules: bool,
}

impl Modes {
    /// The first mode flag set. Destructured without `..`, so a new mode cannot be left out.
    fn flag(&self) -> Option<&'static str> {
        let Modes { fix, list_rules } = self;
        [(*fix, "--fix"), (*list_rules, "--list-rules")]
            .into_iter()
            .find_map(|(set, flag)| set.then_some(flag))
    }
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Replace another scanner's GitHub Action steps with the crosgha Action.
    Migrate {
        /// Scanner whose steps are replaced.
        #[arg(long, value_enum)]
        from: migrate::Tool,

        /// Action reference to insert, `owner/repo@<40-char SHA>`. Without it, the latest
        /// crosgha release is looked up with GH_TOKEN or GITHUB_TOKEN.
        #[arg(long, requires = "action_version")]
        action: Option<String>,

        /// Version comment written after `--action`, such as `v0.1.0`.
        #[arg(long)]
        action_version: Option<String>,

        /// Workflow file or directory to migrate.
        #[arg(value_name = "PATH", default_value = ".github/workflows")]
        path: PathBuf,
    },
    /// Install or remove a git pre-push hook that scans the workflows before every push.
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
}

impl Commands {
    fn name(&self) -> &'static str {
        match self {
            Commands::Migrate { .. } => "migrate",
            Commands::Hook { .. } => "hook",
        }
    }
}

#[derive(Debug, Subcommand)]
enum HookAction {
    /// Write the pre-push hook; arguments after `--` are passed to every scan.
    Install {
        #[arg(last = true, value_name = "SCAN_ARGS")]
        args: Vec<String>,
    },
    /// Remove the pre-push hook if crosgha installed it.
    Uninstall,
}

pub fn run() -> ExitCode {
    let args = Args::parse();
    match args.color {
        // zizmor honors FORCE_COLOR (https://docs.zizmor.sh/usage/, "Cargo-style output"); like
        // anstream's CLICOLOR_FORCE, a non-empty value forces color unless NO_COLOR is set.
        ColorChoice::Auto
            if std::env::var_os("FORCE_COLOR").is_some_and(|value| !value.is_empty())
                && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty()) =>
        {
            anstream::ColorChoice::Always.write_global()
        }
        ColorChoice::Auto => {}
        ColorChoice::Always => anstream::ColorChoice::Always.write_global(),
        ColorChoice::Never => anstream::ColorChoice::Never.write_global(),
    }
    match &args.command {
        Some(Commands::Migrate {
            from,
            action,
            action_version,
            path,
        }) => return run_migrate(*from, action.clone().zip(action_version.clone()), path),
        Some(Commands::Hook { action }) => return run_hook(action),
        None => {}
    }
    if args.modes.list_rules {
        return match args.format {
            None | Some(report::Format::Text) => emit(&report::rules(), ExitCode::SUCCESS),
            Some(report::Format::Json) => emit(&report::rules_json(), ExitCode::SUCCESS),
            Some(_) => fail("--list-rules prints text or json"),
        };
    }
    match scan(&args) {
        Ok(mut scan) => {
            if args.modes.fix {
                write_fixes(&mut scan.files, &scan.errors);
            }
            finish(&args, &scan)
        }
        Err(error) => fail(error),
    }
}

/// Reports `error` and exits 2.
fn fail(error: impl Display) -> ExitCode {
    report::error(error);
    ExitCode::from(2)
}

/// What a scan found, and what kept it from checking everything.
struct Scan {
    files: Vec<FileReport>,
    repository: Vec<Located>,
    errors: Vec<RunError>,
}

fn scan(args: &Args) -> Result<Scan, String> {
    // Each workflow, `.github`, and Dependabot file is paired with the index of its input path.
    let (dependabot, inputs): (Vec<_>, Vec<_>) = args
        .paths
        .iter()
        .cloned()
        .zip(0..)
        .partition(|(path, _)| filesystem::is_dependabot(path));
    let workflows = discover(&inputs)?;
    let token = args
        .gh_token
        .clone()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .filter(|token| !token.is_empty());
    let (mut configs, owners) = load_configs(args)?;
    for id in configs.iter().flat_map(rules::unused_options) {
        report::warning(format_args!(
            "`rules.{id}.config` has no crosgha equivalent and is ignored"
        ));
    }
    let owned = |items: Vec<(PathBuf, usize)>| -> Vec<(PathBuf, usize)> {
        items
            .into_iter()
            .map(|(path, input)| (path, owners[input]))
            .collect()
    };
    let workflows = owned(workflows);
    let mut settings = policy::Settings {
        persona: if args.pedantic {
            policy::Persona::Pedantic
        } else {
            args.persona
        },
        github: token
            .filter(|_| !args.offline && !args.no_online_audits)
            .map(|token| GitHub::new(&api_url(&args.gh_hostname), token)),
        config: config::Config::default(),
        ignores: !args.no_ignores,
        min_severity: args.min_severity.as_deref().and_then(config::severity),
        hosted: api_url(&args.gh_hostname).eq_ignore_ascii_case("https://api.github.com"),
    };
    let inspections = per_config(
        &workflows,
        &mut configs,
        &mut settings,
        |paths, settings| engine::inspect_files(paths, args.modes.fix, settings),
    );
    let mut files = Vec::new();
    let mut errors = Vec::new();
    for ((path, _), inspected) in workflows.into_iter().zip(inspections) {
        match inspected {
            Ok(inspection) => files.push(FileReport {
                path,
                inspection,
                unwritten: None,
            }),
            Err(message) => {
                report::error(format_args!("{}: {message}", path.display()));
                errors.push(RunError::File { path, message });
            }
        }
    }
    let github_dirs = owned(github_dirs(&args.paths));
    let dependabot = outside(owned(dependabot), &github_dirs);
    // Repository rules read the workflows parsed above instead of reading them again.
    let parsed: rules::Scanned = files
        .iter()
        .map(|file| {
            let parsed = rules::Parsed {
                source: &file.inspection.source.text,
                local_actions: &file.inspection.local_actions,
            };
            (filesystem::identity(&file.path), parsed)
        })
        .collect();
    let mut repository = per_config(
        &github_dirs,
        &mut configs,
        &mut settings,
        |dirs, settings| {
            engine::parallel_map(dirs, |github| {
                rules::detect_repository(github, &parsed, settings)
            })
        },
    );
    repository.extend(per_config(
        &dependabot,
        &mut configs,
        &mut settings,
        |paths, settings| {
            paths
                .iter()
                .map(|path| rules::detect_dependabot(path, settings))
                .collect()
        },
    ));
    let mut repository: Vec<Located> = repository
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    // A GitHub API failure (rate limit, 5xx, network, bad token) skips only the online checks that
    // needed the answer; the rest is still reported. The scan is then incomplete and exits 2
    // without writing fixes, so CI does not pass on checks that never ran. A settings read the token
    // is not permitted to make is expected in Actions and only skips `repo-org-settings`.
    if let Some(github) = &settings.github {
        for failure in github.failures() {
            let message = format!("{failure}; online checks that needed it were skipped");
            report::warning(format_args!("{message}, so the scan is incomplete"));
            errors.push(RunError::Skipped(message));
        }
    }
    if let Some(name) = &args.repository {
        let first = [(args.paths[0].clone(), owners[0])];
        let checked = per_config(&first, &mut configs, &mut settings, |scanned, settings| {
            vec![rules::detect_settings(name, &scanned[0], settings)]
        });
        for result in checked {
            match result {
                Ok(found) => repository.extend(found),
                Err(error) => {
                    let message = format!("repo-org-settings was skipped: {error}");
                    if error.denied() {
                        report::warning(&message);
                    } else {
                        report::warning(format_args!("{message}; the scan is incomplete"));
                        errors.push(RunError::Skipped(message));
                    }
                }
            }
        }
    }
    anchor(&mut repository, &files);
    Ok(Scan {
        files,
        repository,
        errors,
    })
}

/// The workflows of every input, each scanned once.
fn discover(inputs: &[(PathBuf, usize)]) -> Result<Vec<(PathBuf, usize)>, String> {
    let mut workflows = Vec::new();
    for (path, input) in inputs {
        let found = filesystem::discover(path)?;
        workflows.extend(found.into_iter().map(|workflow| (workflow, *input)));
    }
    workflows.sort();
    let mut seen = HashSet::new();
    workflows.retain(|(path, _)| seen.insert(filesystem::identity(path)));
    // A link to another scanned workflow is reported, and fixed, once through its target.
    workflows.retain(|(path, _)| {
        filesystem::link_target(path).is_none_or(|target| !seen.contains(&target))
    });
    Ok(workflows)
}

/// The `.github` directory of each input that has one, each once.
fn github_dirs(paths: &[PathBuf]) -> Vec<(PathBuf, usize)> {
    let mut github_dirs: Vec<(PathBuf, usize)> = paths
        .iter()
        .zip(0..)
        .filter_map(|(path, input)| filesystem::github_dir(path).map(|github| (github, input)))
        .collect();
    github_dirs.sort();
    let mut seen = HashSet::new();
    github_dirs.retain(|(github, _)| seen.insert(filesystem::identity(github)));
    github_dirs
}

/// Dependabot files outside the scanned `.github` directories, whose repository rules already
/// check them.
fn outside(
    dependabot: Vec<(PathBuf, usize)>,
    github_dirs: &[(PathBuf, usize)],
) -> Vec<(PathBuf, usize)> {
    let scanned: Vec<PathBuf> = github_dirs
        .iter()
        .filter_map(|(github, _)| github.canonicalize().ok())
        .collect();
    dependabot
        .into_iter()
        .filter(|(path, _)| {
            path.canonicalize()
                .ok()
                .and_then(|path| path.parent().map(Path::to_path_buf))
                .is_none_or(|parent| !scanned.contains(&parent))
        })
        .collect()
}

/// Places each repository-wide finding on the first scanned workflow inside its directory, in
/// path order.
fn anchor(repository: &mut [Located], files: &[FileReport]) {
    for located in repository {
        if let Place::Repository { anchor } = &mut located.place {
            let directory = filesystem::identity(&located.path);
            *anchor = files
                .iter()
                .find(|file| filesystem::identity(&file.path).starts_with(&directory))
                .map(|file| file.path.clone());
        }
    }
}

/// A workflow that failed to parse or an incomplete scan blocks every write, so those runs leave
/// every file as it was and report the original findings. A failed write still exits 2, after
/// the other files were written.
fn write_fixes(files: &mut [FileReport], errors: &[RunError]) {
    let blocked = if errors
        .iter()
        .any(|error| matches!(error, RunError::File { .. }))
    {
        Some("not written because a workflow failed to parse")
    } else if !errors.is_empty() {
        Some("not written because the scan is incomplete")
    } else {
        None
    };
    for file in files {
        let Some(outcome) = file
            .inspection
            .fix
            .as_ref()
            .filter(|outcome| outcome.applied() > 0)
        else {
            continue;
        };
        file.unwritten = match blocked {
            Some(reason) => Some(Unwritten::Blocked(reason.to_owned())),
            None => filesystem::replace(&file.path, &file.inspection.source, &outcome.source)
                .err()
                .map(|error| {
                    report::error(format_args!("{}: {error}", file.path.display()));
                    Unwritten::Failed(error)
                }),
        };
    }
}

/// Writes the reports and picks the exit status: 2 for a run failure, 1 for findings at the
/// `--fail-on` level, 0 otherwise.
fn finish(args: &Args, scan: &Scan) -> ExitCode {
    let Scan {
        files,
        repository,
        errors,
    } = scan;
    #[allow(
        clippy::disallowed_methods,
        reason = "writes the SARIF report, not a workflow"
    )]
    if let Some(target) = &args.sarif_file
        && let Err(error) = std::fs::write(target, report::sarif(files, repository, errors))
    {
        return fail(format_args!("{}: {error}", target.display()));
    }
    let format = args.format.unwrap_or(
        if std::env::var_os("GITHUB_ACTIONS").is_some_and(|value| value == "true") {
            report::Format::Github
        } else {
            report::Format::Text
        },
    );
    let findings = files
        .iter()
        .flat_map(|file| file.shown().1)
        .chain(repository.iter().map(|located| &located.finding));
    let status = if !report::failures(files, errors).is_empty() {
        ExitCode::from(2)
    } else if findings
        .map(|finding| finding.severity)
        .any(|severity| args.fail_on.matches(severity))
    {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    };
    let report = report::render(format, files, repository, errors, args.modes.fix);
    emit(&report, status)
}

/// Prints `text` and returns `status`. A reader that closed the pipe (`| head`) wanted no more
/// output, so that ends the run quietly; any other write failure exits 2.
fn emit(text: &str, status: ExitCode) -> ExitCode {
    let mut stdout = anstream::stdout();
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => status,
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => status,
        Err(error) => {
            report::error(format_args!("cannot write the report: {error}"));
            ExitCode::from(2)
        }
    }
}

/// Configs and, for each input path, the index of its config. `--config` (or its environment
/// variables) applies to every input; otherwise each input discovers its own, so the result never
/// depends on the order of the inputs. Inputs that find the same file share one entry.
fn load_configs(args: &Args) -> Result<(Vec<config::Config>, Vec<usize>), String> {
    let explicit = args.config.clone().or_else(|| {
        ["CROSGHA_CONFIG", "ZIZMOR_CONFIG"]
            .iter()
            .filter_map(std::env::var_os)
            .find(|value| !value.is_empty())
            .map(PathBuf::from)
    });
    if args.no_config || explicit.is_some() {
        let config = match explicit.filter(|_| !args.no_config) {
            Some(path) => config::Config::load(&path)?,
            None => config::Config::default(),
        };
        return Ok((vec![config], vec![0; args.paths.len()]));
    }
    let mut found: Vec<Option<PathBuf>> = Vec::new();
    let mut configs = Vec::new();
    let mut owners = Vec::new();
    for input in &args.paths {
        let discovered = config::Config::discover(input)?;
        let path = discovered.as_ref().map(|(path, _)| path.clone());
        let owner = match found.iter().position(|known| *known == path) {
            Some(owner) => owner,
            None => {
                found.push(path);
                configs.push(discovered.map(|(_, config)| config).unwrap_or_default());
                configs.len() - 1
            }
        };
        owners.push(owner);
    }
    Ok((configs, owners))
}

/// Runs `run` once per config on the items it owns, with that config in `settings`. Results keep
/// the order of `items`.
fn per_config<T: Clone, R>(
    items: &[(T, usize)],
    configs: &mut [config::Config],
    settings: &mut policy::Settings,
    run: impl Fn(&[T], &policy::Settings) -> Vec<R>,
) -> Vec<R> {
    let mut results = Vec::with_capacity(items.len());
    for (owner, config) in configs.iter_mut().enumerate() {
        let (positions, group): (Vec<usize>, Vec<T>) = items
            .iter()
            .enumerate()
            .filter(|(_, (_, of))| *of == owner)
            .map(|(position, (item, _))| (position, item.clone()))
            .unzip();
        if group.is_empty() {
            continue;
        }
        std::mem::swap(&mut settings.config, config);
        results.extend(positions.into_iter().zip(run(&group, settings)));
        std::mem::swap(&mut settings.config, config);
    }
    results.sort_by_key(|(position, _)| *position);
    results.into_iter().map(|(_, result)| result).collect()
}

/// `github.com` uses api.github.com; Enterprise Server hosts serve the API under `/api/v3`.
/// A full URL is used as given. Host names compare case-insensitively, without a trailing slash.
fn api_url(host: &str) -> String {
    let host = host.trim_end_matches('/');
    if host.contains("://") {
        host.to_owned()
    } else if host.eq_ignore_ascii_case("github.com") {
        "https://api.github.com".to_owned()
    } else {
        format!("https://{host}/api/v3")
    }
}

fn run_hook(action: &HookAction) -> ExitCode {
    let result = match action {
        HookAction::Install { args } => {
            // A typo stored in the hook would block every later push, and anything but a plain
            // scan would edit files, always pass, or remove the hook on its first run.
            let parsed = Args::try_parse_from(
                std::iter::once("crosgha").chain(args.iter().map(String::as_str)),
            );
            let culprit = match &parsed {
                Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => Some("--help"),
                Err(error) if error.kind() == clap::error::ErrorKind::DisplayVersion => {
                    Some("--version")
                }
                Err(_) => None,
                Ok(parsed) => match &parsed.command {
                    Some(command) => Some(command.name()),
                    None => parsed.modes.flag(),
                },
            };
            match (parsed, culprit) {
                (_, Some(culprit)) => Err(format!(
                    "`{culprit}` cannot be stored in the pre-push hook, which only scans; \
                     pass scan options such as --offline or --persona"
                )),
                (Err(error), None) => Err(error.to_string()),
                (Ok(_), None) => {
                    hook::install(args).map(|hook| format!("Installed {}.", hook.display()))
                }
            }
        }
        HookAction::Uninstall => hook::uninstall().map(|removed| match removed {
            Some(hook) => format!("Removed {}.", hook.display()),
            None => "No pre-push hook to remove.".to_owned(),
        }),
    };
    match result {
        Ok(message) => emit(&format!("{message}\n"), ExitCode::SUCCESS),
        Err(error) => fail(error),
    }
}

fn run_migrate(
    tool: migrate::Tool,
    action: Option<(String, String)>,
    path: &std::path::Path,
) -> ExitCode {
    let resolved = match action {
        Some(action) => Ok(action),
        None => latest_release(),
    };
    let result =
        resolved.and_then(|(action, version)| migrate::run(path, tool, &action, Some(&version)));
    match result {
        Ok(summary) => {
            for note in &summary.notes {
                report::note(note);
            }
            let mut text = format!(
                "Migrated {} step(s) in {} workflow file(s).\n",
                summary.steps, summary.files
            );
            if let Some(config) = &summary.config {
                text.push_str(&format!("Wrote {}.\n", config.display()));
            }
            emit(&text, ExitCode::SUCCESS)
        }
        Err(error) => fail(error),
    }
}

/// `owner/repo@sha` and tag of the latest crosgha release, from Cargo.toml's `repository`.
fn latest_release() -> Result<(String, String), String> {
    let repository = env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/");
    let token = ["GH_TOKEN", "GITHUB_TOKEN"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|token| !token.is_empty()))
        .ok_or(
            "pass --action and --action-version, or set GH_TOKEN to look up the latest release",
        )?;
    let host = std::env::var("GH_HOST").unwrap_or_else(|_| "github.com".to_owned());
    let github = GitHub::new(&api_url(&host), token);
    let release = github
        .get(&format!("/repos/{repository}/releases/latest"))?
        .ok_or_else(|| format!("{repository} has no release yet"))?;
    let tag = release
        .get("tag_name")
        .and_then(|tag| tag.as_str())
        .ok_or("release has no tag")?
        .to_owned();
    let commit = github
        .get(&format!("/repos/{repository}/commits/{tag}"))?
        .ok_or_else(|| format!("tag {tag} not found"))?;
    let sha = commit
        .get("sha")
        .and_then(|sha| sha.as_str())
        .ok_or("tag has no commit")?;
    Ok((format!("{repository}@{sha}"), tag))
}
