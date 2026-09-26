use std::{path::PathBuf, process::ExitCode};

use clap::Parser;

use crate::{engine, filesystem, policy, report};

#[derive(Debug, Parser)]
#[command(version, about = "Detect and fix GitHub Actions workflow smells.")]
struct Args {
    /// Workflow file or directory to scan.
    #[arg(value_name = "PATH", default_value = ".github/workflows")]
    path: PathBuf,

    /// Apply verified fixes to workflow files.
    #[arg(long)]
    fix: bool,

    /// Require remote actions and workflows to use commit SHAs, and images to use digests.
    #[arg(long)]
    require_immutable_references: bool,

    /// Require timeout-minutes on executable jobs.
    #[arg(long)]
    require_job_timeout: bool,

    /// Require explicit GITHUB_TOKEN permissions at the workflow or job level.
    #[arg(long)]
    require_explicit_token_permissions: bool,

    /// Reject secrets: inherit on reusable workflow calls.
    #[arg(long)]
    require_explicit_workflow_secrets: bool,

    /// Report format. JSON and SARIF are reserved for the planned Action.
    #[arg(long, value_enum, default_value_t = report::Format::Text)]
    format: report::Format,

    /// Minimum severity that makes a scan fail (any, note, warning, error, or none).
    #[arg(long, value_enum, default_value_t = policy::FailOn::Any)]
    fail_on: policy::FailOn,
}

pub fn run() -> ExitCode {
    let args = Args::parse();
    if let Err(error) = args.format.ensure_available() {
        eprintln!("error: {error}");
        return ExitCode::from(2);
    }
    let workflows = match filesystem::discover(&args.path) {
        Ok(workflows) => workflows,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    let mut files = Vec::new();
    let mut failures = 0;
    let rule_policy = policy::RulePolicy {
        require_immutable_references: args.require_immutable_references,
        require_job_timeout: args.require_job_timeout,
        require_explicit_token_permissions: args.require_explicit_token_permissions,
        require_explicit_workflow_secrets: args.require_explicit_workflow_secrets,
    };
    for path in workflows {
        let inspected = filesystem::read(&path, args.fix)
            .and_then(|source| engine::inspect(source, args.fix, rule_policy));
        match inspected {
            Ok(inspection) => files.push(report::FileReport { path, inspection }),
            Err(error) => {
                failures += 1;
                eprintln!("ERROR {}: {error}", path.display());
            }
        }
    }
    if failures > 0 {
        return ExitCode::FAILURE;
    }

    if args.fix {
        for file in &files {
            if let Some(outcome) = &file.inspection.fix
                && outcome.fixed_count > 0
                && let Err(error) = filesystem::write_verified(&file.path, &outcome.source)
            {
                eprintln!("ERROR {}: {error}", file.path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    print!("{}", report::text(&files, args.fix));

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
            .any(|finding| args.fail_on.matches(finding.rule.severity))
    });
    if should_fail {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
