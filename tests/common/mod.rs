//! A fake GitHub release: this build's binary in cargo-dist's archive layout, served over HTTP.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub const ASSET: &str = "crosgha-x86_64-unknown-linux-gnu.tar.xz";

pub fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("crosgha-action-{label}-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A release directory for `version` with the archive and the release-wide `sha256.sum`, optionally
/// tampered.
pub fn release(version: &str, tamper: bool) -> PathBuf {
    let root = temp_dir("release");
    let staging = root.join("crosgha-x86_64-unknown-linux-gnu");
    fs::create_dir_all(&staging).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_crosgha"), staging.join("crosgha")).unwrap();
    let tag = root.join(format!("v{version}"));
    fs::create_dir_all(&tag).unwrap();
    let status = Command::new("tar")
        .env("XZ_OPT", "-0 -T0")
        .arg("-cJf")
        .arg(tag.join(ASSET))
        .arg("-C")
        .arg(&root)
        .arg("crosgha-x86_64-unknown-linux-gnu")
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
