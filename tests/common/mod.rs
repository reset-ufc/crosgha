//! Shared by the end-to-end tests: the built binary, fixtures, scratch directories, and a fake
//! GitHub release (this build's binary in cargo-dist's archive layout, served over HTTP).
// Each test crate uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const ASSET: &str = "crosgha-x86_64-unknown-linux-musl.tar.xz";

/// The built binary, without the variables that would change what a scan does here.
pub fn bin() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crosgha"));
    command
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_ACTIONS")
        .env_remove("GITHUB_REPOSITORY")
        .env_remove("FORCE_COLOR");
    isolate_git(&mut command);
    command
}

/// Run from a git hook or `rebase --exec`, git exports these, and a test's `git init` would then
/// rewrite the developer's repository instead of a scratch one.
pub fn isolate_git(command: &mut Command) -> &mut Command {
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_PREFIX",
    ] {
        command.env_remove(name);
    }
    command
}

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// `(header, path:line:col)` of each `rule` diagnostic in a text report.
pub fn diagnostics<'a>(report: &'a str, rule: &str) -> Vec<(&'a str, &'a str)> {
    let header = format!("[{rule}]: ");
    let lines: Vec<&str> = report.lines().collect();
    lines
        .windows(2)
        .filter(|pair| pair[0].contains(&header))
        .map(|pair| (pair[0], pair[1].trim_start().trim_start_matches("--> ")))
        .collect()
}

/// A new, empty directory under the system temp dir.
pub fn temp_dir(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("crosgha-{label}-{}-{sequence}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A `temp_dir` removed with its contents when dropped, so a failing assertion leaves nothing
/// behind.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        Self(temp_dir(label))
    }
}

impl std::ops::Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A release directory for `version` with the archive and the release-wide `sha256.sum`, optionally
/// tampered.
pub fn release(version: &str, tamper: bool) -> PathBuf {
    package(
        version,
        &fs::read(env!("CARGO_BIN_EXE_crosgha")).unwrap(),
        tamper,
    )
}

/// A release with a matching checksum whose `crosgha` is `script`: a binary that crashes or
/// cannot run on the host, which no checksum catches.
pub fn broken_release(version: &str, script: &str) -> PathBuf {
    package(version, script.as_bytes(), false)
}

fn package(version: &str, binary: &[u8], tamper: bool) -> PathBuf {
    let root = temp_dir("release");
    let staging = root.join("crosgha-x86_64-unknown-linux-musl");
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join("crosgha"), binary).unwrap();
    fs::set_permissions(staging.join("crosgha"), fs::Permissions::from_mode(0o755)).unwrap();
    let tag = root.join(format!("v{version}"));
    fs::create_dir_all(&tag).unwrap();
    let status = Command::new("tar")
        .env("XZ_OPT", "-0 -T0")
        .arg("-cJf")
        .arg(tag.join(ASSET))
        .arg("-C")
        .arg(&root)
        .arg("crosgha-x86_64-unknown-linux-musl")
        .status()
        .unwrap();
    assert!(status.success());
    let digest = Command::new("sha256sum")
        .arg(tag.join(ASSET))
        .output()
        .unwrap();
    let hash = String::from_utf8(digest.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let hash = if tamper { "0".repeat(64) } else { hash };
    // Other targets' lines come first, so the entry point must pick its own asset's line.
    let other = "1".repeat(64);
    fs::write(
        tag.join("sha256.sum"),
        format!("{other}  crosgha-aarch64-apple-darwin.tar.xz\n{hash}  {ASSET}\n"),
    )
    .unwrap();
    root
}

/// Serves files under `root` over HTTP, as GitHub release downloads do.
pub fn serve(root: PathBuf) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
            }
            let path = request
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .trim_start_matches('/');
            match fs::read(root.join(path)) {
                Ok(body) => {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(&body);
                }
                Err(_) => {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                }
            }
        }
    });
    format!("http://{address}")
}
