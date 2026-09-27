use std::{fs, path::PathBuf, process::Command};

const MARKER: &str =
    "# Installed by `crosgha hook install`; remove it with `crosgha hook uninstall`.";

/// Writes the pre-push hook, replacing only a hook crosgha installed itself.
pub fn install(args: &[String]) -> Result<PathBuf, String> {
    let hook = hook_path()?;
    refuse_foreign(&hook)?;
    let args: Vec<String> = args.iter().map(|arg| quote(arg)).collect();
    // ponytail: scans the working tree, not the pushed commits, so uncommitted edits count.
    // Upgrade path: read the pushed refs from stdin and scan each in a temporary worktree.
    let script = format!(
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
"#,
        args = args.iter().map(|arg| format!(" {arg}")).collect::<String>()
    );
    if let Some(dir) = hook.parent() {
        fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    fs::write(&hook, script).map_err(|error| format!("{}: {error}", hook.display()))?;
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
    let hook = hook_path()?;
    if !hook.exists() {
        return Ok(None);
    }
    refuse_foreign(&hook)?;
    fs::remove_file(&hook).map_err(|error| format!("{}: {error}", hook.display()))?;
    Ok(Some(hook))
}

fn refuse_foreign(hook: &std::path::Path) -> Result<(), String> {
    match fs::read_to_string(hook) {
        Ok(script) if !script.contains(MARKER) => Err(format!(
            "{} exists and was not installed by crosgha; add `crosgha` to it by hand",
            hook.display()
        )),
        _ => Ok(()),
    }
}

/// Git resolves `core.hooksPath` and worktrees; the path is relative to the current directory.
fn hook_path() -> Result<PathBuf, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let hooks = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok(PathBuf::from(hooks).join("pre-push"))
}

fn quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}
