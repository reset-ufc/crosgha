use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const BOM: &str = "\u{feff}";

/// Workflow files of a file input, or the `.yml`/`.yaml` files directly inside a directory.
/// GitHub reads workflows only from `.github/workflows` itself: "You must store workflow files in
/// the `.github/workflows` directory of your repository"
/// (https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax), and
/// "Subdirectories of the `workflows` directory are not supported"
/// (https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows). So no
/// directory is walked, and a repository root or its `.github` is scanned only through its
/// workflows, as zizmor collects `.` inputs; the rest of a repository holds YAML that is not a
/// workflow.
pub fn discover(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut workflows = Vec::new();
    let redirected = if is_github_dir(path) {
        Some(path.join("workflows"))
    } else if path.join(".github").is_dir() || path.join(".git").exists() {
        Some(path.join(".github/workflows"))
    } else {
        None
    };
    let path = redirected.as_deref().unwrap_or(path);
    // A repository without `.github/workflows` has no workflow, which the error below says.
    if redirected.is_none() || path.is_dir() {
        collect(path, &mut workflows)?;
    }
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

        let is_file = entry_type.is_file()
            || (entry_type.is_symlink()
                && entry.path().metadata().is_ok_and(|meta| meta.is_file()));
        if is_file && is_yaml_file(&entry.path()) {
            workflows.push(entry.path());
        }
    }
    Ok(())
}

/// One key for every spelling of a path (relative, absolute, through `.`), so overlapping inputs
/// are scanned once. Only the directory is resolved: a symlinked workflow stays its own entry.
pub fn identity(path: &Path) -> PathBuf {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    match (fs::canonicalize(parent), path.file_name()) {
        (Ok(parent), Some(name)) => parent.join(name),
        _ => path.to_path_buf(),
    }
}

/// The `identity` of the file a symbolic link resolves to; `None` for any other path.
pub fn link_target(path: &Path) -> Option<PathBuf> {
    let link = fs::symlink_metadata(path).ok()?.file_type().is_symlink();
    link.then(|| fs::canonicalize(path).ok()).flatten()
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

/// A YAML file's text without its UTF-8 BOM. GitHub accepts the BOM, but the YAML scanner counts
/// it as a column and then rejects a key on the first line, so it is read off here and `replace`
/// puts it back.
pub struct Source {
    pub text: String,
    pub bom: bool,
}

pub fn read(path: &Path) -> io::Result<Source> {
    let text = fs::read_to_string(path)?;
    Ok(match text.strip_prefix(BOM) {
        Some(rest) => Source {
            text: rest.to_owned(),
            bom: true,
        },
        None => Source { text, bom: false },
    })
}

/// Renaming over a link would replace it with a regular file and leave its target unfixed.
pub fn refuse_link(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        true => Err("refusing to replace a symbolic link".to_owned()),
        false => Ok(()),
    }
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Writes `text` to `path` with the BOM `original` had, unless `path` no longer holds `original`,
/// the content `text` was computed from.
pub fn replace(path: &Path, original: &Source, text: &str) -> Result<(), String> {
    refuse_link(path)?;
    let name = path
        .file_name()
        .ok_or("workflow has no filename")?
        .to_string_lossy();
    let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_file_name(format!(
        ".{name}.crosgha-{}-{sequence}.tmp",
        std::process::id()
    ));
    let bom = if original.bom { BOM } else { "" };
    let cannot = |error: io::Error| format!("cannot write verified fix: {error}");
    let result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(cannot)?;
        file.set_permissions(fs::metadata(path).map_err(cannot)?.permissions())
            .map_err(cannot)?;
        file.write_all(bom.as_bytes()).map_err(cannot)?;
        file.write_all(text.as_bytes()).map_err(cannot)?;
        file.sync_all().map_err(cannot)?;
        // An edit saved during the scan would be lost. An edit between this check and the rename
        // still is; only a lock every editor honors could close that window.
        let current = fs::read(path).map_err(cannot)?;
        if current.strip_prefix(bom.as_bytes()) != Some(original.text.as_bytes()) {
            return Err("file changed since it was read".to_owned());
        }
        #[allow(clippy::disallowed_methods, reason = "the verified write itself")]
        fs::rename(&temporary, path).map_err(cannot)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
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
