use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::workflow::BOM;

pub fn discover(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut workflows = Vec::new();
    // A repository root or its `.github` is scanned through its workflows, as zizmor collects
    // `.` inputs; `.github` also holds YAML that is not a workflow.
    let workflows_dir = if is_github_dir(path) {
        path.join("workflows")
    } else {
        path.join(".github/workflows")
    };
    let path = if workflows_dir.is_dir() {
        workflows_dir.as_path()
    } else {
        path
    };
    collect(path, &mut workflows)?;
    workflows.sort();
    if workflows.is_empty() {
        return Err(format!(
            "no .yml or .yaml workflow files found in `{}`",
            path.display()
        ));
    }
    Ok(workflows)
}

fn collect(path: &Path, workflows: &mut Vec<PathBuf>) -> Result<(), String> {
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
            collect(&entry.path(), workflows)?;
            continue;
        }
        // ponytail: Symlinked files are scanned like direct paths; symlinked
        // directories are not followed, which avoids cycle detection for now.
        let is_file = entry_type.is_file()
            || (entry_type.is_symlink()
                && entry.path().metadata().is_ok_and(|meta| meta.is_file()));
        if is_file && is_yaml_file(&entry.path()) {
            workflows.push(entry.path());
        }
    }
    Ok(())
}

/// A Dependabot config shares the `.yml` extension but is not a workflow.
pub fn is_dependabot(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .is_some_and(|name| name == "dependabot.yml" || name == "dependabot.yaml")
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("yml" | "yaml")
    )
}

pub fn read(path: &Path, fix: bool) -> Result<String, String> {
    if fix
        && fs::symlink_metadata(path)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("refusing to replace a symbolic link".to_owned());
    }
    fs::read_to_string(path).map_err(|error| format!("cannot read workflow: {error}"))
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub fn write_verified(path: &Path, source: &str) -> Result<(), String> {
    let bom = fs::read(path)
        .is_ok_and(|old| old.starts_with(BOM.encode_utf8(&mut [0; 3]).as_bytes()))
        && !source.starts_with(BOM);
    let name = path
        .file_name()
        .ok_or("workflow has no filename")?
        .to_string_lossy();
    let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_file_name(format!(
        ".{name}.crosgha-{}-{sequence}.tmp",
        std::process::id()
    ));
    let result = (|| -> Result<(), std::io::Error> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.set_permissions(fs::metadata(path)?.permissions())?;
        if bom {
            write!(file, "{BOM}")?;
        }
        file.write_all(source.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| format!("cannot write verified fix: {error}"))
}

/// The `.github` directory that owns a scanned `.github/workflows` directory or repository root.
pub fn github_dir(scanned: &Path) -> Option<PathBuf> {
    if !scanned.is_dir() {
        return None;
    }
    if is_github_dir(scanned) {
        return Some(scanned.to_path_buf());
    }
    let parent = scanned.parent().filter(|parent| {
        scanned.file_name().is_some_and(|name| name == "workflows")
            && parent.file_name().is_some_and(|name| name == ".github")
    });
    parent
        .map(Path::to_path_buf)
        .or_else(|| Some(scanned.join(".github")).filter(|github| github.is_dir()))
}

fn is_github_dir(path: &Path) -> bool {
    path.is_dir() && path.file_name().is_some_and(|name| name == ".github")
}
