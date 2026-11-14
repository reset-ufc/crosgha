//! The files of one repository revision, in a local directory or read through the GitHub API, so
//! the repository rules, action discovery, and config discovery have one body for both kinds of
//! input.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::github::GitHub;
use crate::workflow::Kind;
use crate::{filesystem, report};

/// What a path of a tree names. A remote tree's symbolic links and submodules are neither: the
/// API lists a link's target only as blob content, and a submodule's files are in another
/// repository.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Entry {
    File,
    Directory,
}

/// A repository's files. Paths are relative to the repository root and separated by `/`, as Git
/// stores them, whatever the platform; a backslash is part of a name.
pub trait Tree: Sync {
    /// The path reports name for `path`.
    fn path(&self, path: &str) -> PathBuf;

    fn entry(&self, path: &str) -> Result<Option<Entry>, String>;

    /// The text of the file at `path` without a UTF-8 BOM; `None` when it is not a file.
    fn read(&self, path: &str) -> Result<Option<String>, String>;

    /// The names of the files directly inside `directory` (`""` for the root), sorted.
    fn files(&self, directory: &str) -> Result<Vec<String>, String>;

    /// A key that every spelling of the file at `path` shares, or `None` when it resolves outside
    /// the repository.
    fn identity(&self, path: &str) -> Option<PathBuf>;

    fn is_file(&self, path: &str) -> Result<bool, String> {
        Ok(self.entry(path)? == Some(Entry::File))
    }
}

/// The source of each file the scan already read, by `filesystem::identity` of its path, so
/// repository rules do not read it again.
pub type Scanned<'a> = HashMap<PathBuf, &'a str>;

/// A repository checked out in a local directory. Symbolic links are followed.
pub struct LocalTree<'a> {
    root: PathBuf,
    scanned: &'a Scanned<'a>,
}

impl<'a> LocalTree<'a> {
    pub fn new(root: PathBuf, scanned: &'a Scanned<'a>) -> Self {
        Self { root, scanned }
    }
}

impl Tree for LocalTree<'_> {
    fn path(&self, path: &str) -> PathBuf {
        if path.is_empty() {
            self.root.clone()
        } else {
            self.root.join(path)
        }
    }

    fn entry(&self, path: &str) -> Result<Option<Entry>, String> {
        let metadata = fs::metadata(self.path(path)).ok();
        Ok(match metadata {
            Some(metadata) if metadata.is_file() => Some(Entry::File),
            Some(metadata) if metadata.is_dir() => Some(Entry::Directory),
            _ => None,
        })
    }

    fn read(&self, path: &str) -> Result<Option<String>, String> {
        let full = self.path(path);
        if let Some(source) = self.scanned.get(&filesystem::identity(&full)) {
            return Ok(Some((*source).to_owned()));
        }
        if !full.is_file() {
            return Ok(None);
        }
        filesystem::read(&full)
            .map(|source| Some(source.text))
            .map_err(|error| format!("cannot read `{}`: {error}", full.display()))
    }

    fn files(&self, directory: &str) -> Result<Vec<String>, String> {
        let Ok(entries) = fs::read_dir(spelled(&self.path(directory))) else {
            return Ok(Vec::new());
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        names.sort();
        Ok(names)
    }

    fn identity(&self, path: &str) -> Option<PathBuf> {
        let root = fs::canonicalize(spelled(&self.root)).ok()?;
        let resolved = fs::canonicalize(self.path(path)).ok()?;
        resolved.starts_with(root).then_some(resolved)
    }
}

/// `path`, or `.` for the empty path of the current directory, which the file system rejects.
fn spelled(path: &Path) -> &Path {
    if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    }
}

/// One revision of a GitHub repository, read through the cached client. Every path of a
/// repository comes from one recursive tree request
/// (https://docs.github.com/en/rest/git/trees#get-a-tree). When GitHub truncates it (over
/// 100,000 entries or 7 MB, per that page), each directory a lookup needs is listed on its own,
/// from the root down; the rules look only in the root, `.github`, and the directories of local
/// actions, so that stays a few requests.
pub struct RemoteTree<'a> {
    github: &'a GitHub,
    slug: String,
    sha: String,
    /// The root tree's SHA, where per-directory listings start.
    root: String,
    /// Every directory's entries from the recursive listing; `None` when it was truncated.
    listed: Option<HashMap<String, Vec<Listed>>>,
}

#[derive(Clone)]
struct Listed {
    name: String,
    entry: Option<Entry>,
    sha: String,
}

impl<'a> RemoteTree<'a> {
    /// Lists `slug` (`owner/repo`) at the commit `sha`.
    pub fn new(github: &'a GitHub, slug: &str, sha: &str) -> Result<Self, String> {
        let path = format!("/repos/{slug}/git/trees/{sha}?recursive=1");
        let tree = github
            .get(&path)?
            .ok_or_else(|| format!("GitHub API {path}: tree not found"))?;
        let root = tree["sha"]
            .as_str()
            .filter(|root| is_sha(root))
            .ok_or_else(|| format!("GitHub API {path}: expected a tree SHA"))?
            .to_owned();
        let listed = if tree["truncated"] == true {
            None
        } else {
            let mut directories: HashMap<String, Vec<Listed>> = HashMap::new();
            directories.insert(String::new(), Vec::new());
            for (full, listed) in entries(&path, &tree)? {
                if listed.entry == Some(Entry::Directory) {
                    directories.entry(full.clone()).or_default();
                }
                let (parent, _) = split(&full);
                directories
                    .entry(parent.to_owned())
                    .or_default()
                    .push(listed);
            }
            Some(directories)
        };
        Ok(Self {
            github,
            slug: slug.to_owned(),
            sha: sha.to_owned(),
            root,
            listed,
        })
    }

    /// The entries of `directory`, a normalized path; `None` when it is not a directory.
    fn listing(&self, directory: &str) -> Result<Option<Vec<Listed>>, String> {
        if let Some(listed) = &self.listed {
            return Ok(listed.get(directory).cloned());
        }
        let sha = if directory.is_empty() {
            self.root.clone()
        } else {
            let (parent, name) = split(directory);
            let found = self.listing(parent)?.and_then(|entries| {
                entries
                    .into_iter()
                    .find(|listed| listed.name == name && listed.entry == Some(Entry::Directory))
            });
            match found {
                Some(listed) => listed.sha,
                None => return Ok(None),
            }
        };
        let path = format!("/repos/{}/git/trees/{sha}", self.slug);
        let tree = self
            .github
            .get(&path)?
            .ok_or_else(|| format!("GitHub API {path}: tree not found"))?;
        if tree["truncated"] == true {
            return Err(format!(
                "GitHub API {path}: the directory listing is truncated"
            ));
        }
        let entries = entries(&path, &tree)?;
        Ok(Some(
            entries.into_iter().map(|(_, listed)| listed).collect(),
        ))
    }
}

impl Tree for RemoteTree<'_> {
    fn path(&self, path: &str) -> PathBuf {
        PathBuf::from(format!("{}@{}:{path}", self.slug, self.sha))
    }

    fn entry(&self, path: &str) -> Result<Option<Entry>, String> {
        let Some(path) = normalize(path) else {
            return Ok(None);
        };
        if path.is_empty() {
            return Ok(Some(Entry::Directory));
        }
        let (parent, name) = split(&path);
        let entries = self.listing(parent)?.unwrap_or_default();
        Ok(entries
            .into_iter()
            .find(|listed| listed.name == name)
            .and_then(|listed| listed.entry))
    }

    fn read(&self, path: &str) -> Result<Option<String>, String> {
        let Some(path) = normalize(path) else {
            return Ok(None);
        };
        if !self.is_file(&path)? {
            return Ok(None);
        }
        // The tree lists the file, so a 404 means the revision changed under the scan.
        let text = self
            .github
            .contents(&self.slug, &self.sha, &path)?
            .ok_or_else(|| format!("GitHub API: {} disappeared", self.path(&path).display()))?;
        Ok(Some(text.trim_start_matches('\u{feff}').to_owned()))
    }

    fn files(&self, directory: &str) -> Result<Vec<String>, String> {
        let Some(directory) = normalize(directory) else {
            return Ok(Vec::new());
        };
        let mut names: Vec<String> = self
            .listing(&directory)?
            .unwrap_or_default()
            .into_iter()
            .filter(|listed| listed.entry == Some(Entry::File))
            .map(|listed| listed.name)
            .collect();
        names.sort();
        Ok(names)
    }

    fn identity(&self, path: &str) -> Option<PathBuf> {
        normalize(path).map(|path| self.path(&path))
    }
}

/// `(path, entry)` for each item of a tree response; a recursive listing's paths start at the
/// root, a single directory's are its names.
fn entries(api_path: &str, tree: &Value) -> Result<Vec<(String, Listed)>, String> {
    let items = tree["tree"]
        .as_array()
        .ok_or_else(|| format!("GitHub API {api_path}: expected a tree"))?;
    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        let (Some(path), Some(sha)) = (item["path"].as_str(), item["sha"].as_str()) else {
            return Err(format!(
                "GitHub API {api_path}: tree entry without path or SHA"
            ));
        };
        // Modes are Git's: 040000 trees, 100644/100755 blobs, 120000 links, 160000 submodules.
        let entry = match (item["type"].as_str(), item["mode"].as_str()) {
            (Some("tree"), _) if is_sha(sha) => Some(Entry::Directory),
            (Some("blob"), Some(mode)) if mode != "120000" => Some(Entry::File),
            _ => None,
        };
        let (_, name) = split(path);
        let listed = Listed {
            name: name.to_owned(),
            entry,
            sha: sha.to_owned(),
        };
        entries.push((path.to_owned(), listed));
    }
    Ok(entries)
}

/// Hexadecimal, so a SHA from a response is safe inside the next request's path.
fn is_sha(sha: &str) -> bool {
    !sha.is_empty() && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// `path` without `.` and empty segments, with each `..` applied; `None` when it leaves the root.
pub fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// The parent directory and the name of `path`.
fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

pub fn join(directory: &str, name: &str) -> String {
    match directory.trim_end_matches('/') {
        "" => name.to_owned(),
        directory => format!("{directory}/{name}"),
    }
}

/// Whether the file at `path` is a workflow or an action, as `filesystem::kind` tells them apart.
pub fn kind(path: &str) -> Kind {
    let (parent, name) = split(path);
    let workflows = normalize(parent).is_some_and(|parent| parent == ".github/workflows");
    if (name == "action.yml" || name == "action.yaml") && !workflows {
        Kind::Action
    } else {
        Kind::Workflow
    }
}

/// The metadata file of the action in `directory`. The runner loads `action.yml` and reads
/// `action.yaml` only when there is none (`ActionManager.cs` in actions/runner at
/// 8cfa1049f86781d141056082d73802d56cd2b433); a directory with only a `Dockerfile` has none.
pub fn action_file(tree: &dyn Tree, directory: &str) -> Result<Option<String>, String> {
    for name in ["action.yml", "action.yaml"] {
        let path = join(directory, name);
        if tree.is_file(&path)? {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Finds the actions a repository runs or publishes: the action at its root and each local action
/// in `called` (the paths after `uses: ./` in its scanned files), followed through the actions
/// they call until none is new. Paths resolve from the root, the caller's checkout: "an action
/// defined in the same repository as the workflow"
/// (https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax). zizmor
/// walks every directory for `action.yml` instead; this finds only the actions the repository
/// uses or publishes, and others are scanned when given as inputs.
///
/// `seen` holds the `Tree::identity` of every file already scanned, so each action is scanned
/// once, a cycle or a link to a known action included. `inspect` scans each batch of new metadata
/// files and returns the local actions each one calls.
pub fn actions(
    tree: &dyn Tree,
    called: Vec<String>,
    seen: &mut HashSet<PathBuf>,
    mut inspect: impl FnMut(&[String]) -> Vec<Vec<String>>,
) -> Result<(), String> {
    let mut pending = vec![String::new()];
    pending.extend(called);
    let mut warned = HashSet::new();
    while !pending.is_empty() {
        let mut batch = Vec::new();
        for directory in pending.drain(..) {
            let Some(path) = action_file(tree, &directory)? else {
                continue;
            };
            match tree.identity(&path) {
                Some(identity) => {
                    if seen.insert(identity) {
                        batch.push(path);
                    }
                }
                // `..` or a symbolic link may lead out of the repository, to files it does not own.
                None => {
                    if warned.insert(path.clone()) {
                        report::warning(format_args!(
                            "{}: the local action resolves outside its repository and is not \
                             scanned",
                            tree.path(&path).display()
                        ));
                    }
                }
            }
        }
        pending.extend(inspect(&batch).into_iter().flatten());
    }
    Ok(())
}
