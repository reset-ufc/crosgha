use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const MARKER: &str =
    "# Installed by `crosgha hook install`; remove it with `crosgha hook uninstall`.";

/// Writes the pre-push hook, replacing only a hook crosgha installed itself.
pub fn install(args: &[String]) -> Result<PathBuf, String> {
    let (hook, repository) = locate()?;
    let dir = hook.parent().unwrap_or(&hook);
    let resolved = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_owned());
    if !repository.iter().any(|root| resolved.starts_with(root)) {
        return Err(format!(
            "git resolves hooks to {}, outside this repository (core.hooksPath is probably set \
             globally); a hook there would run crosgha on every push from every repository \
             that shares it. Set a hooks directory for this repository, such as \
             `git config core.hooksPath .githooks`, or call crosgha from the shared hook yourself",
            dir.display()
        ));
    }
    refuse_foreign(&hook)?;
    fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    fs::write(&hook, script(args)).map_err(|error| format!("{}: {error}", hook.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("{}: {error}", hook.display()))?;
    }
    Ok(hook)
}

/// Removes the pre-push hook if crosgha installed it; `None` when there was none.
pub fn uninstall() -> Result<Option<PathBuf>, String> {
    let (hook, _) = locate()?;
    if fs::symlink_metadata(&hook).is_err() {
        return Ok(None);
    }
    refuse_foreign(&hook)?;
    fs::remove_file(&hook).map_err(|error| format!("{}: {error}", hook.display()))?;
    Ok(Some(hook))
}

fn script(args: &[String]) -> String {
    let args: String = args.iter().map(|arg| format!(" {}", quote(arg))).collect();
    // ponytail: scans the working tree, not the pushed commits, so uncommitted edits count.
    // Upgrade path: read the pushed refs from stdin and scan each in a temporary worktree.
    format!(
        r#"#!/bin/sh
{MARKER}
if ! command -v crosgha >/dev/null 2>&1; then
  echo "pre-push: crosgha is not on PATH; install it or delete $0" >&2
  exit 1
fi
cd "$(git rev-parse --show-toplevel)" || exit 1
crosgha{args}
status=$?
if [ "$status" -eq 1 ]; then
  echo "pre-push: fix the findings above ('crosgha --fix' applies the verified fixes) or skip them with 'git push --no-verify'" >&2
fi
exit "$status"
"#
    )
}

fn refuse_foreign(hook: &Path) -> Result<(), String> {
    // Anything that exists but cannot be read as our script (non-UTF-8, a directory,
    // unreadable, a dangling symlink) is someone else's hook too.
    match fs::read_to_string(hook) {
        Ok(script) if script.contains(MARKER) => Ok(()),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && fs::symlink_metadata(hook).is_err() =>
        {
            Ok(())
        }
        _ => Err(format!(
            "{} exists and was not installed by crosgha; add `crosgha` to it by hand",
            hook.display()
        )),
    }
}

/// The pre-push hook git would run, after `core.hooksPath` and worktrees, and the directories
/// that belong to this repository: its working tree and the git directory worktrees share.
fn locate() -> Result<(PathBuf, Vec<PathBuf>), String> {
    let output = Command::new("git")
        .args([
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "hooks",
            "--show-toplevel",
            "--git-common-dir",
        ])
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut paths = stdout.lines().map(PathBuf::from);
    let hook = paths.next().unwrap_or_default().join("pre-push");
    let repository = paths
        .map(|root| fs::canonicalize(&root).unwrap_or(root))
        .collect();
    Ok((hook, repository))
}

fn quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}
