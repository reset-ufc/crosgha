use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::Parser;
use yamlpath::{Document, FeatureKind, Route};

#[derive(Debug, Parser)]
#[command(about = "Parse GitHub Actions workflows; smell rules are not implemented yet.")]
struct Args {
    /// Workflow file or directory to scan.
    #[arg(value_name = "PATH", default_value = ".github/workflows")]
    path: PathBuf,
}

fn main() -> ExitCode {
    run(Args::parse().path)
}

fn run(path: PathBuf) -> ExitCode {
    let mut workflows = Vec::new();

    if let Err(error) = collect_workflows(&path, &mut workflows) {
        eprintln!("error: {error}");
        return ExitCode::from(2);
    }

    workflows.sort();
    if workflows.is_empty() {
        eprintln!(
            "error: no .yml or .yaml workflow files found in `{}`",
            path.display()
        );
        return ExitCode::from(2);
    }

    let mut failures = 0;
    for workflow in &workflows {
        match check_workflow(workflow) {
            Ok(()) => {}
            Err(error) => {
                failures += 1;
                eprintln!("ERROR {}: {error}", workflow.display());
            }
        }
    }

    println!(
        "Checked {} workflow file(s); smell detection and fixes are not implemented yet.",
        workflows.len()
    );

    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn collect_workflows(path: &Path, workflows: &mut Vec<PathBuf>) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("cannot access `{}`: {error}", path.display()))?;

    if metadata.is_file() {
        if is_yaml_file(path) {
            workflows.push(path.to_path_buf());
            return Ok(());
        }
        return Err(format!("`{}` is not a .yml or .yaml file", path.display()));
    }

    if !metadata.is_dir() {
        return Err(format!("`{}` is not a file or directory", path.display()));
    }

    let entries =
        fs::read_dir(path).map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot read entry in `{}`: {error}", path.display()))?;
        let entry_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect `{}`: {error}", entry.path().display()))?;

        if entry_type.is_dir() {
            collect_workflows(&entry.path(), workflows)?;
        } else if entry_type.is_file() && is_yaml_file(&entry.path()) {
            workflows.push(entry.path());
        }
    }

    Ok(())
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("yml" | "yaml")
    )
}

fn check_workflow(path: &Path) -> Result<(), String> {
    let source =
        fs::read_to_string(path).map_err(|error| format!("cannot read workflow: {error}"))?;
    let document = Document::new(source).map_err(|error| error.to_string())?;
    let root = Route::default();
    for key in ["on", "jobs"] {
        if !document.query_exists(&root.with_key(key)) {
            return Err(format!("missing required top-level key `{key}`"));
        }
    }

    let jobs = document
        .query_exact(&root.with_key("jobs"))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "top-level `jobs` has no value".to_string())?;
    if !matches!(
        jobs.kind(),
        FeatureKind::BlockMapping | FeatureKind::FlowMapping
    ) {
        return Err("top-level `jobs` must be a mapping".to_string());
    }

    let (line, column) = jobs.location.point_span.0;
    println!(
        "OK {}: YAML parsed; `on` and `jobs` found; `jobs` starts at {}:{}",
        path.display(),
        line + 1,
        column + 1
    );
    Ok(())
}
