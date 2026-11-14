use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::workflow::Kind;

const BOM: &str = "\u{feff}";

/// Workflow and action files of a file input, or the `.yml`/`.yaml` files directly inside a
/// directory. GitHub reads workflows only from `.github/workflows` itself: "You must store
/// workflow files in the `.github/workflows` directory of your repository"
/// (https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax), and
/// "Subdirectories of the `workflows` directory are not supported"
/// (https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows). So no
/// directory is walked, and a repository root or its `.github` is scanned only through its
/// workflows, as zizmor collects `.` inputs; the rest of a repository holds YAML that is not a
/// workflow. The actions of a repository are found from its workflows instead
/// (`repository_root`, `action_file`), so an input whose repository publishes an action at its
/// root is not empty.
pub fn discover(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut workflows = Vec::new();
    let redirected = if is_github_dir(path) {
        Some(path.join("workflows"))
    } else if path.join(".github").is_dir() || path.join(".git").exists() {
        Some(path.join(".github/workflows"))
    } else {
        None
    };
    let directory = redirected.as_deref().unwrap_or(path);
    // A repository without `.github/workflows` has no workflow, which the error below says.
    if redirected.is_none() || directory.is_dir() {
        collect(directory, &mut workflows)?;
    }
    workflows.sort();
    let root_action = repository_root(path).and_then(|root| action_file(&root));
    if workflows.is_empty() && root_action.is_none() {
        return Err(format!(
            "no .yml or .yaml workflow files found in `{}`",
            directory.display()
        ));
    }
    Ok(workflows)
}

/// Whether `path` is an action metadata file. As in zizmor, a file named `action.yml` or
/// `action.yaml` is an action unless it sits in `.github/workflows`, where it is a workflow
/// (`collect_from_file` in `crates/zizmor/src/registry/input.rs` at
/// dc2209b6b6d76781c43d2a09aa9d51b56627457c).
pub fn kind(path: &Path) -> Kind {
    let named = path
        .file_name()
        .is_some_and(|name| name == "action.yml" || name == "action.yaml");
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let in_workflows =
        fs::canonicalize(parent).is_ok_and(|parent| parent.ends_with(".github/workflows"));
    if named && !in_workflows {
        Kind::Action
    } else {
        Kind::Workflow
    }
}

/// The metadata file of the action in `directory`. The runner loads `action.yml` and reads
/// `action.yaml` only when there is none (`ActionManager.cs` in actions/runner at
/// 8cfa1049f86781d141056082d73802d56cd2b433); a directory with only a `Dockerfile` has none.
pub fn action_file(directory: &Path) -> Option<PathBuf> {
    ["action.yml", "action.yaml"]
        .iter()
        .map(|name| directory.join(name))
        .find(|path| path.is_file())
}

/// The repository an input belongs to: the root above a `.github/workflows` directory or a
/// `.github` directory, or a directory with `.github` or `.git`. `None` for files and other
/// directories.
pub fn repository_root(input: &Path) -> Option<PathBuf> {
    let parent = |path: &Path| match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    if !input.is_dir() {
        return None;
    }
    if is_github_dir(input) {
        return Some(parent(input));
    }
    let workflows = input.file_name().is_some_and(|name| name == "workflows")
        && input
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == ".github");
    if workflows {
        return Some(parent(&parent(input)));
    }
    (input.join(".github").is_dir() || input.join(".git").exists()).then(|| input.to_path_buf())
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
        // The runner never reads an `action.yaml` next to an `action.yml` (`action_file`).
        let shadowed = entry.file_name() == "action.yaml"
            && path.join("action.yml").is_file()
            && kind(&entry.path()) == Kind::Action;
        if is_file && is_yaml_file(&entry.path()) && !shadowed {
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

/// The directory a scan reaches files from when `input` is one of its paths: the input itself, or
/// the directory of a file input.
pub fn base(input: &Path) -> PathBuf {
    match input.parent() {
        _ if input.is_dir() => input.to_path_buf(),
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Refuses a write to `path` through a symbolic link: the file itself, which renaming would replace
/// with a regular file and leave its target unfixed, or a directory between `base` and the file,
/// which may lead out of the tree the scan started from (another repository, or a directory the
/// user never named). A link above `base`, such as a symlinked home directory, is the user's.
pub fn refuse_link(path: &Path, base: &Path) -> Result<(), String> {
    let is_link = |path: &Path| {
        fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
    };
    if is_link(path) {
        return Err("refusing to replace a symbolic link".to_owned());
    }
    let directories = path
        .ancestors()
        .skip(1)
        .take_while(|ancestor| *ancestor != base && !ancestor.as_os_str().is_empty());
    for directory in directories {
        if is_link(directory) {
            return Err(format!(
                "refusing to write through the symbolic link `{}`",
                directory.display()
            ));
        }
    }
    Ok(())
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Writes `text` to `path` with the BOM `original` had, unless `path` no longer holds `original`,
/// the content `text` was computed from, or a symbolic link leads to it below `base`
/// (`refuse_link`).
pub fn replace(path: &Path, base: &Path, original: &Source, text: &str) -> Result<(), String> {
    refuse_link(path, base)?;
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
