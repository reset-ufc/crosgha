#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! `crosgha hook install|uninstall` in scratch git repositories.

mod common;

use common::{TempDir, bin, fixture, isolate_git};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// A throwaway repository, `work` next to a bare `remote.git`, whose git ignores the user's
/// global and system config (hooksPath, signing). It lives as long as the returned `TempDir`.
fn git_repo(name: &str) -> (TempDir, PathBuf) {
    let temp = TempDir::new(&format!("hook-{name}"));
    git(&temp, &["init", "-q", "--bare", "remote.git"]);
    git(&temp, &["init", "-q", "work"]);
    let work = temp.join("work");
    (temp, work)
}

fn git(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let bin_dir = PathBuf::from(env!("CARGO_BIN_EXE_crosgha"))
        .parent()
        .unwrap()
        .to_path_buf();
    let path = std::env::join_paths(
        std::iter::once(bin_dir).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    isolate_git(&mut Command::new("git"))
        .current_dir(dir)
        .args(args)
        .env("PATH", path)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_ACTIONS")
        .output()
        .unwrap()
}

fn combined(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn pre_push_hook_blocks_smelly_workflows_until_they_are_fixed() {
    let (_repo, work) = git_repo("push");
    // Spaces and quotes in the scanned path prove the stored arguments survive shell quoting.
    let flows = work.join("ci flows");
    fs::create_dir_all(&flows).unwrap();
    fs::copy(
        fixture("checkout-credentials.before.yml"),
        flows.join("it's.yml"),
    )
    .unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-qm", "ci"]);
    fs::create_dir_all(work.join("src")).unwrap();

    let install = bin()
        .current_dir(work.join("src"))
        .args(["hook", "install", "--", "--offline", "ci flows"])
        .output()
        .unwrap();
    assert!(install.status.success(), "{}", combined(&install));
    let hook = work.join(".git/hooks/pre-push");
    assert!(
        combined(&install).contains("pre-push"),
        "{}",
        combined(&install)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(fs::metadata(&hook).unwrap().permissions().mode() & 0o111, 0);
    }

    let remote = work.parent().unwrap().join("remote.git");
    let remote_url = remote.to_str().unwrap();
    let blocked = git(
        &work.join("src"),
        &["push", remote_url, "HEAD:refs/heads/main"],
    );
    assert!(!blocked.status.success(), "{}", combined(&blocked));
    let report = combined(&blocked);
    assert!(report.contains("[persist-credentials]"), "{report}");
    assert!(report.contains("crosgha --fix"), "{report}");
    assert!(
        !git(&remote, &["rev-parse", "--verify", "-q", "main"])
            .status
            .success()
    );

    // Without crosgha on PATH the hook fails closed instead of letting the push through.
    let missing = isolate_git(&mut Command::new("git"))
        .current_dir(&work)
        .args(["push", remote_url, "HEAD:refs/heads/main"])
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(
        combined(&missing).contains("crosgha is not on PATH"),
        "{}",
        combined(&missing)
    );

    let fixed = bin()
        .current_dir(&work)
        .args(["--offline", "--fix", "ci flows"])
        .output()
        .unwrap();
    assert!(fixed.status.success(), "{}", combined(&fixed));
    git(&work, &["commit", "-qam", "fix"]);
    let pushed = git(&work, &["push", remote_url, "HEAD:refs/heads/main"]);
    assert!(pushed.status.success(), "{}", combined(&pushed));
    assert!(
        git(&remote, &["rev-parse", "--verify", "-q", "main"])
            .status
            .success()
    );

    let uninstall = bin()
        .current_dir(&work)
        .args(["hook", "uninstall"])
        .output()
        .unwrap();
    assert!(uninstall.status.success(), "{}", combined(&uninstall));
    assert!(!hook.exists());
}

#[test]
fn hook_install_follows_hooks_path_and_never_touches_foreign_hooks() {
    let (_repo, work) = git_repo("foreign");
    git(&work, &["config", "core.hooksPath", ".githooks"]);
    fs::create_dir_all(work.join(".githooks")).unwrap();
    let foreign = work.join(".githooks/pre-push");
    // Not valid UTF-8: a hook we cannot even read as text is still someone else's.
    fs::write(&foreign, b"#!/bin/sh\nmake check # \xff\n").unwrap();

    for command in [&["hook", "install"][..], &["hook", "uninstall"][..]] {
        let refused = bin().current_dir(&work).args(command).output().unwrap();
        assert_eq!(refused.status.code(), Some(2), "{}", combined(&refused));
        assert!(
            combined(&refused).contains(".githooks/pre-push"),
            "{}",
            combined(&refused)
        );
        assert_eq!(
            fs::read(&foreign).unwrap(),
            b"#!/bin/sh\nmake check # \xff\n"
        );
    }
    assert!(!work.join(".git/hooks/pre-push").exists());

    fs::remove_file(&foreign).unwrap();
    std::os::unix::fs::symlink("/nonexistent/pre-push", &foreign).unwrap();
    let dangling = bin()
        .current_dir(&work)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert_eq!(dangling.status.code(), Some(2), "{}", combined(&dangling));
    assert!(fs::symlink_metadata(&foreign).unwrap().is_symlink());

    let typo = bin()
        .current_dir(&work)
        .args(["hook", "install", "--", "--persona", "paranoid"])
        .output()
        .unwrap();
    assert_eq!(typo.status.code(), Some(2), "{}", combined(&typo));
    assert!(combined(&typo).contains("paranoid"), "{}", combined(&typo));

    fs::remove_file(&foreign).unwrap();
    for persona in ["pedantic", "auditor"] {
        let installed = bin()
            .current_dir(&work)
            .args(["hook", "install", "--", "--persona", persona])
            .output()
            .unwrap();
        assert!(installed.status.success(), "{}", combined(&installed));
    }
    let hook = fs::read_to_string(&foreign).unwrap();
    assert!(
        hook.contains("'auditor'") && !hook.contains("pedantic"),
        "{hook}"
    );

    let outside = std::env::temp_dir();
    let no_repo = bin()
        .current_dir(&outside)
        .env("GIT_CEILING_DIRECTORIES", &outside)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert_eq!(no_repo.status.code(), Some(2), "{}", combined(&no_repo));
}

#[test]
fn hook_install_rejects_arguments_that_stop_it_from_being_a_read_only_scan() {
    let (_repo, work) = git_repo("args");
    let hook = work.join(".git/hooks/pre-push");
    for (args, culprit) in [
        (&["--offline", "--fix", "--no-config"][..], "--fix"),
        (&["migrate", "--from", "zizmor"][..], "migrate"),
        (&["hook", "uninstall"][..], "hook"),
        (&["--list-rules"][..], "--list-rules"),
        (&["--version"][..], "--version"),
        (&["-h"][..], "--help"),
    ] {
        let install = bin()
            .current_dir(&work)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(["hook", "install", "--"])
            .args(args)
            .output()
            .unwrap();
        let output = combined(&install);
        assert_eq!(install.status.code(), Some(2), "{args:?}: {output}");
        assert!(output.contains(culprit), "{args:?}: {output}");
        assert!(!hook.exists(), "{args:?}");
    }
}

#[test]
fn hook_install_stays_inside_the_repository_it_was_run_in() {
    let (_repo, work) = git_repo("shared");
    let root = work.parent().unwrap().to_path_buf();
    git(&work, &["commit", "-q", "--allow-empty", "-m", "init"]);

    // A global core.hooksPath is shared by every repository on the machine.
    let shared = root.join("shared-hooks");
    let global = root.join("gitconfig");
    fs::write(
        &global,
        format!("[core]\n\thooksPath = {}\n", shared.display()),
    )
    .unwrap();
    let refused = bin()
        .current_dir(&work)
        .env("GIT_CONFIG_GLOBAL", &global)
        .args(["hook", "install"])
        .output()
        .unwrap();
    let output = combined(&refused);
    assert_eq!(refused.status.code(), Some(2), "{output}");
    assert!(
        output.contains("core.hooksPath") && output.contains("shared-hooks"),
        "{output}"
    );
    assert!(!shared.exists());

    // Hooks git keeps in the main repository are fine for a linked worktree.
    git(&work, &["worktree", "add", "-q", "../linked"]);
    let linked = bin()
        .current_dir(root.join("linked"))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert!(linked.status.success(), "{}", combined(&linked));
    let ours = work.join(".git/hooks/pre-push");
    assert!(ours.exists());

    // A hook an earlier release left in the shared directory can still be removed.
    fs::create_dir_all(&shared).unwrap();
    fs::copy(&ours, shared.join("pre-push")).unwrap();
    let removed = bin()
        .current_dir(&work)
        .env("GIT_CONFIG_GLOBAL", &global)
        .args(["hook", "uninstall"])
        .output()
        .unwrap();
    assert!(removed.status.success(), "{}", combined(&removed));
    assert!(!shared.join("pre-push").exists());
}

#[test]
fn hook_install_treats_an_edited_copy_of_its_script_as_foreign() {
    let (_repo, work) = git_repo("copy");
    let run = |args: &[&str]| {
        bin()
            .current_dir(&work)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(args)
            .output()
            .unwrap()
    };
    let installed = run(&["hook", "install", "--", "--offline"]);
    assert!(installed.status.success(), "{}", combined(&installed));
    let hook = work.join(".git/hooks/pre-push");
    let generated = fs::read_to_string(&hook).unwrap();

    // What a user who follows "add `crosgha` to it by hand" ends up with.
    for edited in [
        format!("{generated}make check\n"),
        generated.replace("crosgha '--offline'", "crosgha '--offline' && make check"),
    ] {
        fs::write(&hook, &edited).unwrap();
        for command in [&["hook", "install"][..], &["hook", "uninstall"][..]] {
            let refused = run(command);
            assert_eq!(refused.status.code(), Some(2), "{}", combined(&refused));
            assert_eq!(fs::read_to_string(&hook).unwrap(), edited);
        }
    }

    // Our own script stays ours, even with quotes inside its stored arguments.
    fs::write(&hook, &generated).unwrap();
    let reinstalled = run(&["hook", "install", "--", "--persona", "pedantic", "a 'b' c"]);
    assert!(reinstalled.status.success(), "{}", combined(&reinstalled));
    let removed = run(&["hook", "uninstall"]);
    assert!(removed.status.success(), "{}", combined(&removed));
    assert!(!hook.exists());
}
