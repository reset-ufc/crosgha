//! Shared by the end-to-end tests: the built binary, fixtures, scratch directories, a fake GitHub
//! API, and a fake GitHub release (this build's binary in cargo-dist's archive layout, served over
//! HTTP).
// Each test crate uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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

/// Copies the directory `from` into `to`, recursively.
pub fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
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
    assert!(
        status.success(),
        "tar could not build the release archive: {status}"
    );
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

/// The token the fake GitHub API accepts; any other gets 401.
pub const TOKEN: &str = "test-token";

/// The commit every repository of `Api::repositories` is served at.
pub const COMMIT: &str = "f00df00df00df00df00df00df00df00df00df00d";

/// An API error the fake server answers instead of the recorded response: the path (`*` for
/// every path), the status line, extra header lines, and GitHub's `message`.
pub type Failure = (&'static str, &'static str, &'static str, &'static str);

/// What the fake GitHub API answers besides the responses recorded in
/// `tests/fixtures/online/api.json`.
pub struct Api {
    pub failures: &'static [Failure],
    /// Every `page=` answers like page 1, as a proxy that drops the parameter.
    pub ignore_page: bool,
    /// Each directory here is the repository `fixture/<name>` at `COMMIT`, served through the
    /// commits, Git trees, and contents endpoints. `.git` is left out, and symbolic links are
    /// listed with Git's link mode.
    pub repositories: PathBuf,
    /// Recursive tree listings hold only their first entry and say `truncated: true`, as GitHub
    /// answers for a tree over its limits.
    pub truncated: bool,
}

impl Default for Api {
    fn default() -> Self {
        Self {
            failures: &[],
            ignore_page: false,
            repositories: fixture("repos"),
            truncated: false,
        }
    }
}

/// Serves `api` on a local port; unknown paths get 404 and a missing token gets 401. The log holds
/// every requested path in arrival order.
pub fn github_api(api: Api) -> (String, Arc<Mutex<Vec<String>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::clone(&log);
    let routes: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&fs::read_to_string(fixture("online/api.json")).unwrap()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            let path = request.split_whitespace().nth(1).unwrap_or("").to_owned();
            requests.lock().unwrap().push(path.clone());
            let mut authorized = false;
            let mut raw = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
                authorized |=
                    header.to_ascii_lowercase().trim() == format!("authorization: bearer {TOKEN}");
                raw |= header
                    .to_ascii_lowercase()
                    .contains("application/vnd.github.raw+json")
                    || header
                        .to_ascii_lowercase()
                        .contains("application/vnd.github.sha");
            }
            let failure = api
                .failures
                .iter()
                .find(|(failing, ..)| *failing == "*" || *failing == path);
            let route = match path.split_once("&page=") {
                Some((before, _)) if api.ignore_page => format!("{before}&page=1"),
                _ => path.clone(),
            };
            let recorded = routes.get(&route).map(|body| match body.as_str() {
                Some(text) if raw => text.to_owned(),
                _ => body.to_string(),
            });
            let answer = recorded.or_else(|| repository(&api.repositories, &path, api.truncated));
            let (status, headers, body) = match (failure, answer) {
                _ if !authorized => (
                    "401 Unauthorized",
                    "",
                    r#"{"message":"Bad credentials"}"#.to_owned(),
                ),
                (Some((_, status, headers, message)), _) => (
                    *status,
                    *headers,
                    serde_json::json!({ "message": message }).to_string(),
                ),
                (None, Some(body)) => ("200 OK", "", body),
                (None, None) => ("404 Not Found", "", r#"{"message":"Not Found"}"#.to_owned()),
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\n{headers}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (format!("http://{address}"), log)
}

/// The answer for `request` about a repository `fixture/<name>` under `root`.
fn repository(root: &Path, request: &str, truncated: bool) -> Option<String> {
    let (name, endpoint) = request.strip_prefix("/repos/fixture/")?.split_once('/')?;
    let repository = root.join(name);
    if name.starts_with('.') || !repository.is_dir() {
        return None;
    }
    if endpoint.starts_with("commits/") {
        return Some(COMMIT.to_owned());
    }
    if let Some(tree) = endpoint.strip_prefix("git/trees/") {
        // A tree's SHA is its path, hex-encoded behind `tree:`, so any directory can be listed.
        let (entries, truncate) = match tree.strip_suffix("?recursive=1") {
            Some(commit) if commit == COMMIT => (listing(&repository, "", true), truncated),
            Some(_) => return None,
            None => {
                let decoded = hex_decode(tree)?;
                let directory = decoded.strip_prefix("tree:")?;
                if !repository.join(directory).is_dir() {
                    return None;
                }
                (listing(&repository, directory, false), false)
            }
        };
        let entries = if truncate {
            entries.into_iter().take(1).collect()
        } else {
            entries
        };
        let body = serde_json::json!({
            "sha": hex_encode("tree:"),
            "tree": entries,
            "truncated": truncate,
        });
        return Some(body.to_string());
    }
    let (file, reference) = endpoint.strip_prefix("contents/")?.split_once("?ref=")?;
    let file = repository.join(percent_decode(file));
    let regular = fs::symlink_metadata(&file).is_ok_and(|metadata| metadata.is_file());
    (reference == COMMIT && regular).then(|| fs::read_to_string(file).unwrap())
}

/// Git tree entries of `directory` in `repository`, with every level below when `recursive`.
fn listing(repository: &Path, directory: &str, recursive: bool) -> Vec<serde_json::Value> {
    let mut names: Vec<String> = fs::read_dir(repository.join(directory))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name != ".git")
        .collect();
    names.sort();
    let mut entries = Vec::new();
    for name in names {
        let path = if directory.is_empty() {
            name.clone()
        } else {
            format!("{directory}/{name}")
        };
        let metadata = fs::symlink_metadata(repository.join(&path)).unwrap();
        let (kind, mode) = if metadata.is_symlink() {
            ("blob", "120000")
        } else if metadata.is_dir() {
            ("tree", "040000")
        } else {
            ("blob", "100644")
        };
        let listed = if recursive { &path } else { &name };
        entries.push(serde_json::json!({
            "path": listed,
            "mode": mode,
            "type": kind,
            "sha": hex_encode(&format!("{kind}:{path}")),
        }));
        if recursive && kind == "tree" {
            entries.extend(listing(repository, &path, true));
        }
    }
    entries
}

fn hex_encode(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

pub fn hex_decode(hex: &str) -> Option<String> {
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}

pub fn percent_decode(text: &str) -> String {
    let mut bytes = Vec::new();
    let mut rest = text.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        let decoded = (byte == b'%')
            .then(|| std::str::from_utf8(tail.get(..2)?).ok())
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match decoded {
            Some(decoded) => {
                bytes.push(decoded);
                rest = &tail[2..];
            }
            None => {
                bytes.push(byte);
                rest = tail;
            }
        }
    }
    String::from_utf8(bytes).unwrap()
}
