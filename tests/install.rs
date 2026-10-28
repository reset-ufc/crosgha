#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! Pipes `install.sh` into `sh`, as `curl -fsSL .../install.sh | sh` does, against a local release
//! server laid out like GitHub's release download URLs.

mod common;

use common::{broken_release, release, serve, temp_dir};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `root` served under both GitHub URL shapes: `download/vX.Y.Z/` and `latest/download/`.
fn github_release(root: PathBuf) -> (PathBuf, String) {
    symlink(".", root.join("download")).unwrap();
    fs::create_dir(root.join("latest")).unwrap();
    symlink(
        root.join(format!("v{VERSION}")),
        root.join("latest/download"),
    )
    .unwrap();
    let base = serve(root.clone());
    (root, base)
}

fn install(base: &str, home: &Path, path: &str, extra: &[(&str, &str)]) -> Output {
    let mut command = Command::new("sh");
    command
        .env_clear()
        .env("PATH", path)
        .env("HOME", home)
        .env("CROSGHA_RELEASE_BASE", base)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra {
        command.env(key, value);
    }
    let mut child = command.spawn().unwrap();
    let script = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh")).unwrap();
    child.stdin.take().unwrap().write_all(&script).unwrap();
    child.wait_with_output().unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn install_puts_the_latest_release_on_the_user_path_and_upgrades_in_place() {
    let (root, base) = github_release(release(VERSION, false));
    let home = temp_dir("home");
    let system_path = std::env::var("PATH").unwrap();
    let bin = home.join(".local/bin");

    let first = install(&base, &home, &system_path, &[]);
    assert!(first.status.success(), "{}", text(&first));
    let version = Command::new(bin.join("crosgha"))
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        format!("crosgha {VERSION}")
    );
    // ~/.local/bin is missing from PATH here, so the user must be told how to add it.
    assert!(text(&first).contains("PATH"), "{}", text(&first));

    // A second run replaces the installed binary; with the directory on PATH there is no hint.
    let on_path = format!("{}:{system_path}", bin.display());
    let second = install(&base, &home, &on_path, &[]);
    assert!(second.status.success(), "{}", text(&second));
    assert!(!text(&second).contains("PATH"), "{}", text(&second));

    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn install_refuses_a_tampered_or_missing_release_and_installs_nothing() {
    let (root, base) = github_release(release(VERSION, true));
    let home = temp_dir("home");
    let path = std::env::var("PATH").unwrap();
    let installed = home.join(".local/bin/crosgha");

    let tampered = install(&base, &home, &path, &[("CROSGHA_VERSION", VERSION)]);
    assert!(!tampered.status.success());
    assert!(text(&tampered).contains("checksum"), "{}", text(&tampered));
    assert!(!installed.exists());

    let missing = install(&base, &home, &path, &[("CROSGHA_VERSION", "v0.0.1")]);
    assert!(!missing.status.success());
    assert!(text(&missing).contains("v0.0.1"), "{}", text(&missing));
    assert!(!installed.exists());

    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn install_keeps_the_working_binary_when_the_new_one_cannot_run() {
    let (good_root, good) = github_release(release(VERSION, false));
    // Correctly signed, yet built for another machine: only running it tells.
    let (bad_root, bad) = github_release(broken_release(
        VERSION,
        "#!/bin/sh\necho 'cannot execute binary file: Exec format error' >&2\nexit 126\n",
    ));
    let home = temp_dir("home");
    let path = std::env::var("PATH").unwrap();
    let bin = home.join(".local/bin");

    let first = install(&good, &home, &path, &[]);
    assert!(first.status.success(), "{}", text(&first));
    let upgrade = install(&bad, &home, &path, &[]);
    assert!(!upgrade.status.success(), "{}", text(&upgrade));
    assert!(
        text(&upgrade).contains("Exec format error"),
        "{}",
        text(&upgrade)
    );

    let version = Command::new(bin.join("crosgha"))
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        format!("crosgha {VERSION}")
    );
    assert_eq!(
        fs::read_dir(&bin).unwrap().count(),
        1,
        "the rejected binary was left behind"
    );

    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(good_root).unwrap();
    fs::remove_dir_all(bad_root).unwrap();
}

#[test]
fn install_uses_plain_http_only_for_a_bare_loopback_origin() {
    let (root, base) = github_release(release(VERSION, false));
    let home = temp_dir("home");
    // Userinfo makes the URL start like the test origin while naming another host.
    let disguised = base.replacen("http://", "http://127.0.0.1:1@", 1);
    let output = install(&disguised, &home, &std::env::var("PATH").unwrap(), &[]);
    assert!(!output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains("cannot download"),
        "{}",
        text(&output)
    );
    assert!(!home.join(".local/bin/crosgha").exists());

    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn install_refuses_platforms_without_a_release_binary() {
    let home = temp_dir("home");
    // A fake `uname` first on PATH reports an Apple silicon Mac.
    let fake = temp_dir("uname");
    let uname = fake.join("uname");
    fs::write(
        &uname,
        "#!/bin/sh\ncase \"$1\" in -s) echo Darwin ;; -m) echo arm64 ;; esac\n",
    )
    .unwrap();
    fs::set_permissions(&uname, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", fake.display(), std::env::var("PATH").unwrap());

    // Nothing listens here: the platform check must fail before any download.
    let output = install("http://127.0.0.1:9", &home, &path, &[]);
    assert!(!output.status.success());
    assert!(text(&output).contains("Darwin arm64"), "{}", text(&output));
    assert!(text(&output).contains("cargo install"), "{}", text(&output));
    assert!(!home.join(".local/bin/crosgha").exists());

    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(fake).unwrap();
}
