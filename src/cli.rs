use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};

use crate::{config, engine, filesystem, github::GitHub, migrate, policy, report, rules};

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

    /// Apply verified fixes to workflow files.
    #[arg(long)]
    fix: bool,

    /// Sensitivity: regular (default), pedantic (adds code smells), or auditor (everything).
    #[arg(long, value_enum, default_value_t = policy::Persona::Regular)]
    persona: policy::Persona,

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
    #[arg(long, env = "CROSGHA_CONFIG")]
    config: Option<PathBuf>,

    /// Do not discover or load crosgha.yml or zizmor.yml.
    #[arg(long, conflicts_with = "config")]
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

    /// Print every rule with its taxonomy tag, severity, and persona, then exit.
    #[arg(long)]
    list_rules: bool,

    /// Also write a SARIF report to this file, whatever `--format` prints.
    #[arg(long, value_name = "FILE")]
    sarif_file: Option<PathBuf>,

    /// Report format: text (also `plain`), json, sarif, or github (workflow annotations).
    #[arg(long, value_enum, default_value_t = report::Format::Text)]
    format: report::Format,

    /// Minimum severity that makes a scan fail (any, note, warning, error, or none).
    #[arg(long, value_enum, default_value_t = policy::FailOn::Any)]
    fail_on: policy::FailOn,
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
}

pub fn run() -> ExitCode {
    let args = Args::parse();
    if let Some(Commands::Migrate {
        from,
        action,
        action_version,
        path,
    }) = &args.command
    {
        return run_migrate(*from, action.clone().zip(action_version.clone()), path);
    }
    if args.list_rules {
        print!("{}", report::rules());
        return ExitCode::SUCCESS;
    }
    let workflows = match args
        .paths
        .iter()
        .map(|path| filesystem::discover(path))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(found) => {
            let mut workflows: Vec<PathBuf> = found.into_iter().flatten().collect();
            workflows.sort();
            workflows.dedup();
            workflows
        }
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    let mut files = Vec::new();
    let mut failures = 0;
    let token = args
        .gh_token
        .clone()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .filter(|token| !token.is_empty());
    let config = match load_config(&args) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    for id in rules::unused_options(&config) {
        eprintln!("warning: `rules.{id}.config` has no crosgha equivalent and is ignored");
    }
    let settings = policy::Settings {
        persona: args.persona,
        github: token
            .filter(|_| !args.offline && !args.no_online_audits)
            .map(|token| GitHub::new(&api_url(&args.gh_hostname), token)),
        config,
        ignores: !args.no_ignores,
        min_severity: args.min_severity.as_deref().and_then(config::severity),
    };
    for path in workflows {
        let file = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let inspected = filesystem::read(&path, args.fix)
            .and_then(|source| engine::inspect(source, args.fix, &settings, &file));
        match inspected {
            Ok(inspection) => files.push(report::FileReport { path, inspection }),
            Err(error) => {
                failures += 1;
                eprintln!("ERROR {}: {error}", path.display());
            }
        }
    }
    if failures > 0 {
        return ExitCode::from(2);
    }
    let mut github_dirs: Vec<PathBuf> = args
        .paths
        .iter()
        .filter_map(|path| filesystem::github_dir(path))
        .collect();
    github_dirs.sort();
    github_dirs.dedup();
    let repository = github_dirs
        .iter()
        .map(|github| rules::detect_repository(github, &settings))
        .collect::<Result<Vec<_>, _>>()
        .map(|found| found.into_iter().flatten().collect::<Vec<_>>())
        .and_then(|mut found| {
            if let Some(repository) = &args.repository {
                found.extend(rules::detect_settings(
                    repository,
                    &args.paths[0],
                    &settings,
                )?);
            }
            Ok(found)
        });
    let repository = match repository {
        Ok(found) => found,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    if args.fix {
        for file in &files {
            if let Some(outcome) = &file.inspection.fix
                && outcome.fixed_count > 0
                && let Err(error) = filesystem::write_verified(&file.path, &outcome.source)
            {
                eprintln!("ERROR {}: {error}", file.path.display());
                return ExitCode::from(2);
            }
        }
    }
    if let Some(target) = &args.sarif_file
        && let Err(error) = std::fs::write(target, report::sarif(&files, &repository, args.fix))
    {
        eprintln!("error: {}: {error}", target.display());
        return ExitCode::from(2);
    }
    print!(
        "{}",
        report::render(args.format, &files, &repository, args.fix)
    );

    let should_fail = files.iter().any(|file| {
        let findings = if args.fix {
            file.inspection
                .fix
                .as_ref()
                .map_or(&[][..], |outcome| outcome.remaining.as_slice())
        } else {
            &file.inspection.findings
        };
        findings
            .iter()
            .any(|finding| args.fail_on.matches(finding.severity))
    });
    let should_fail = should_fail
        || repository
            .iter()
            .any(|located| args.fail_on.matches(located.finding.severity));
    if should_fail {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn load_config(args: &Args) -> Result<config::Config, String> {
    if args.no_config {
        return Ok(config::Config::default());
    }
    let explicit = args
        .config
        .clone()
        .or_else(|| std::env::var_os("ZIZMOR_CONFIG").map(PathBuf::from));
    match explicit {
        Some(path) => config::Config::load(&path),
        None => Ok(config::Config::discover(&args.paths[0])?
            .map(|(_, config)| config)
            .unwrap_or_default()),
    }
}

/// `github.com` uses api.github.com; Enterprise Server hosts serve the API under `/api/v3`.
/// A full URL is used as given.
fn api_url(host: &str) -> String {
    if host.contains("://") {
        host.to_owned()
    } else if host == "github.com" {
        "https://api.github.com".to_owned()
    } else {
        format!("https://{host}/api/v3")
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
                eprintln!("note: {note}");
            }
            println!(
                "Migrated {} step(s) in {} workflow file(s).",
                summary.steps, summary.files
            );
            if let Some(config) = &summary.config {
                println!("Wrote {}.", config.display());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
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
