//! `--follow-actions`: the actions that scanned files call from other repositories
//! (`uses: owner/repo[/path]@ref`), read through the GitHub API at the commit their ref names and
//! checked with crosgha's own step and metadata rules. Composite actions are followed through the
//! actions their steps call, as the runner prepares them (`PrepareActionsRecursiveAsync` in
//! `src/Runner.Worker/ActionManager.cs`, actions/runner at
//! 8cfa1049f86781d141056082d73802d56cd2b433).

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::engine::{self, Inspection};
use crate::filesystem::Source;
use crate::github::{ApiError, GitHub};
use crate::policy::Settings;
use crate::report::{self, FileReport, Link, Reached, RunError};
use crate::rules::{Call, Finding};
use crate::tree;
use crate::workflow::Kind;

/// The runner refuses composite actions nested deeper than this (`CompositeActionsMaxDepth` in
/// `src/Runner.Common/Constants.cs` at the commit above); the action a job's step calls is at
/// depth 0, so an action at this depth is checked and its calls are not followed.
const MAX_DEPTH: usize = 9;

/// One action at one commit. GitHub owner and repository names ignore case, so they are kept
/// lowercase; the path inside the repository is case-sensitive, as Git paths are.
#[derive(Clone, Eq, Hash, PartialEq)]
struct Key {
    owner: String,
    repo: String,
    path: String,
    sha: String,
}

impl Key {
    fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// The name reports give the metadata file `name` of this action, the form a remote input's
    /// files have (`owner/repo@sha:path/action.yml`).
    fn file(&self, name: &str) -> PathBuf {
        PathBuf::from(format!(
            "{}@{}:{}",
            self.slug(),
            self.sha,
            tree::join(&self.path, name)
        ))
    }
}

/// What a `uses:` line names, before its ref is resolved to a commit.
struct Reference {
    owner: String,
    repo: String,
    path: String,
    git_ref: String,
}

impl Reference {
    fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// `owner/repo[/path]@ref`, the same for every spelling of the reference.
    fn name(&self) -> String {
        let path = match self.path.as_str() {
            "" => String::new(),
            path => format!("/{path}"),
        };
        format!("{}{path}@{}", self.slug(), self.git_ref)
    }

    fn sha(&self) -> Option<String> {
        let full =
            self.git_ref.len() == 40 && self.git_ref.bytes().all(|byte| byte.is_ascii_hexdigit());
        full.then(|| self.git_ref.to_ascii_lowercase())
    }

    fn key(&self, sha: String) -> Key {
        Key {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            path: self.path.clone(),
            sha,
        }
    }
}

/// The action `uses` names when the step sits in `parent` (`None` for a scanned file), or `None`
/// when it is not followed: `$/path` outside another repository's action, a path that leaves the
/// repository, or a name or ref that GitHub does not allow, which would also change the request
/// path. A trailing `/` and empty segments in the path name the same directory.
fn reference(uses: &str, parent: Option<&Key>) -> Option<Reference> {
    if let Some(path) = uses.strip_prefix("$/") {
        let parent = parent?;
        return Some(Reference {
            owner: parent.owner.clone(),
            repo: parent.repo.clone(),
            path: tree::normalize(path)?,
            git_ref: parent.sha.clone(),
        });
    }
    let (name, git_ref) = uses.rsplit_once('@')?;
    let mut parts = name.splitn(3, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    let path = tree::normalize(parts.next().unwrap_or_default())?;
    // The characters `cli::remote_input` accepts in `owner/repo`; others could change the request
    // path.
    let allowed = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    };
    // Git refuses `..` and spaces in ref names: https://git-scm.com/docs/git-check-ref-format
    let valid_ref = !git_ref.is_empty()
        && !git_ref.contains("..")
        && !git_ref.bytes().any(|byte| byte.is_ascii_whitespace());
    if !allowed(owner) || !allowed(repo) || matches!(repo, "." | "..") || !valid_ref {
        return None;
    }
    Some(Reference {
        owner: owner.to_ascii_lowercase(),
        repo: repo.to_ascii_lowercase(),
        path,
        git_ref: git_ref.to_owned(),
    })
}

/// Who holds a `uses:` line: a scanned file (an index into the roots) or a followed action.
#[derive(Clone, Copy)]
enum Caller {
    Root(usize),
    Node(usize),
}

/// A `uses:` line still to be followed.
struct Pending {
    caller: Caller,
    /// The index of the line among the caller's calls.
    call: usize,
    depth: usize,
    reference: Reference,
}

/// An action that was read and checked.
struct Node {
    key: Key,
    path: PathBuf,
    inspection: Inspection,
    depth: usize,
}

/// The metadata file read for an action, and its inspection or parse error.
struct Fetched {
    path: PathBuf,
    inspection: Result<Inspection, String>,
}

/// A `uses:` line that leads to a node.
struct Edge {
    caller: Caller,
    call: usize,
    target: usize,
}

/// The actions of other repositories that a scan reached, and what kept it from reading some.
#[derive(Default)]
pub struct Followed {
    pub files: Vec<FileReport>,
    pub errors: Vec<RunError>,
}

/// Reads and checks every action that the calls of `roots` (scanned files) reach, each
/// `(owner, repo, path, sha)` once. `settings` holds the config of the input `roots` come from;
/// its online rules do not run on the fetched files, and their ignore comments are not honored,
/// since another repository wrote them. A finding is dropped only when every scanned `uses:`
/// line that reaches it ignores its rule, inline or in the config.
pub fn follow(github: &GitHub, roots: &[&FileReport], settings: &Settings) -> Followed {
    let checks = Settings {
        persona: settings.persona,
        github: None,
        config: settings.config.clone(),
        ignores: false,
        min_severity: settings.min_severity,
        hosted: settings.hosted,
    };
    let mut walk = Walk {
        github,
        roots,
        settings: &checks,
        nodes: Vec::new(),
        visited: HashMap::new(),
        edges: Vec::new(),
        errors: Vec::new(),
        named: HashSet::new(),
        halted: None,
        unread: 0,
    };
    let mut level = Vec::new();
    for (root, file) in roots.iter().enumerate() {
        for (call, found) in file.inspection.calls.iter().enumerate() {
            if let Some(reference) = reference(&found.uses, None) {
                level.push(Pending {
                    caller: Caller::Root(root),
                    call,
                    depth: 0,
                    reference,
                });
            }
        }
    }
    while !level.is_empty() {
        level = walk.level(level);
    }
    if let Some(error) = walk.halted.take() {
        let message = format!(
            "{error}; {} referenced action(s) were not read",
            walk.unread
        );
        report::warning(format_args!("{message}, so the scan is incomplete"));
        walk.errors.push(RunError::Skipped(message));
    }
    let errors = std::mem::take(&mut walk.errors);
    Followed {
        files: walk.reports(settings),
        errors,
    }
}

struct Walk<'a> {
    github: &'a GitHub,
    roots: &'a [&'a FileReport],
    settings: &'a Settings,
    nodes: Vec<Node>,
    /// Every key looked up, with its node; `None` when it could not be read.
    visited: HashMap<Key, Option<usize>>,
    edges: Vec<Edge>,
    errors: Vec<RunError>,
    /// `Reference::name` of each reference already warned or failed about, so each is named once.
    named: HashSet<String>,
    /// The rate limit or rejected token that stopped every later request.
    halted: Option<ApiError>,
    /// How many distinct references the halt left unread.
    unread: usize,
}

impl Walk<'_> {
    /// Follows one level of `uses:` lines, breadth first, so the first chain that reaches an
    /// action is a shortest one; returns the calls of the actions it read.
    fn level(&mut self, level: Vec<Pending>) -> Vec<Pending> {
        let mut refs: Vec<(String, String)> = Vec::new();
        for pending in &level {
            let named = (pending.reference.slug(), pending.reference.git_ref.clone());
            if pending.reference.sha().is_none() && !refs.contains(&named) {
                refs.push(named);
            }
        }
        let resolved =
            engine::parallel_map(&refs, |(slug, git_ref)| self.github.commit(slug, git_ref));
        let resolved: HashMap<(String, String), Result<Option<String>, ApiError>> =
            refs.into_iter().zip(resolved).collect();
        let mut keys = Vec::with_capacity(level.len());
        let mut new = Vec::new();
        for pending in &level {
            let reference = &pending.reference;
            let sha = match reference.sha() {
                Some(sha) => Some(sha),
                None => match &resolved[&(reference.slug(), reference.git_ref.clone())] {
                    Ok(Some(sha)) => Some(sha.clone()),
                    Ok(None) => {
                        self.warn(
                            pending,
                            "the repository or ref does not exist, so it was not followed",
                        );
                        None
                    }
                    Err(error) => {
                        self.fail(pending, error);
                        None
                    }
                },
            };
            let key = sha.map(|sha| reference.key(sha));
            if let Some(key) = &key
                && !self.visited.contains_key(key)
                && !new.contains(key)
            {
                new.push(key.clone());
            }
            keys.push(key);
        }
        let read = engine::parallel_map(&new, |key| self.read(key));
        let start = self.nodes.len();
        for (key, read) in new.iter().zip(read) {
            let first = level
                .iter()
                .zip(&keys)
                .find(|(_, of)| of.as_ref() == Some(key))
                .map(|(pending, _)| pending)
                .expect("every new key comes from a pending call");
            let node = match read {
                Ok(Some(Fetched {
                    path,
                    inspection: Ok(inspection),
                })) => {
                    self.nodes.push(Node {
                        key: key.clone(),
                        path,
                        inspection,
                        depth: first.depth,
                    });
                    Some(self.nodes.len() - 1)
                }
                Ok(Some(Fetched {
                    path,
                    inspection: Err(message),
                })) => {
                    report::error(format_args!("{}: {message}", path.display()));
                    self.errors.push(RunError::File { path, message });
                    None
                }
                Ok(None) => {
                    self.warn(
                        first,
                        "it has no action.yml or action.yaml at that commit (a Dockerfile-only \
                         action, a wrong path, or a commit outside the repository), so it was \
                         not checked",
                    );
                    None
                }
                Err(error) => {
                    self.fail(first, &error);
                    None
                }
            };
            self.visited.insert(key.clone(), node);
        }
        for (pending, key) in level.iter().zip(&keys) {
            if let Some(Some(target)) = key.as_ref().and_then(|key| self.visited.get(key)) {
                self.edges.push(Edge {
                    caller: pending.caller,
                    call: pending.call,
                    target: *target,
                });
            }
        }
        let mut next = Vec::new();
        for (index, node) in self.nodes.iter().enumerate().skip(start) {
            let calls: Vec<(usize, Reference)> = node
                .inspection
                .calls
                .iter()
                .enumerate()
                .filter_map(|(call, found)| Some((call, reference(&found.uses, Some(&node.key))?)))
                .collect();
            if calls.is_empty() {
                continue;
            }
            if node.depth == MAX_DEPTH {
                report::warning(format_args!(
                    "{}: its {} call(s) nest deeper than the runner's limit of {MAX_DEPTH} \
                     composite actions, so they were not followed",
                    node.path.display(),
                    calls.len()
                ));
                continue;
            }
            for (call, reference) in calls {
                next.push(Pending {
                    caller: Caller::Node(index),
                    call,
                    depth: node.depth + 1,
                    reference,
                });
            }
        }
        next
    }

    /// The metadata file of the action at `key` and its inspection: the runner reads
    /// `action.yml`, then `action.yaml` (`ActionManager.cs` at the commit above).
    fn read(&self, key: &Key) -> Result<Option<Fetched>, ApiError> {
        for name in ["action.yml", "action.yaml"] {
            let found =
                self.github
                    .contents(&key.slug(), &key.sha, &tree::join(&key.path, name))?;
            if let Some(text) = found {
                let source = Source {
                    text: text.trim_start_matches('\u{feff}').to_owned(),
                    bom: false,
                };
                let inspection = engine::inspect(source, Kind::Action, false, self.settings, name);
                return Ok(Some(Fetched {
                    path: key.file(name),
                    inspection,
                }));
            }
        }
        Ok(None)
    }

    /// `path:line` of the `uses:` line `pending` follows.
    fn origin(&self, pending: &Pending) -> String {
        let (path, call) = self.line(pending.caller, pending.call);
        format!("{}:{}", path.display(), call.span[0])
    }

    fn line(&self, caller: Caller, call: usize) -> (&Path, &Call) {
        match caller {
            Caller::Root(root) => (
                &self.roots[root].path,
                &self.roots[root].inspection.calls[call],
            ),
            Caller::Node(node) => (
                &self.nodes[node].path,
                &self.nodes[node].inspection.calls[call],
            ),
        }
    }

    fn warn(&mut self, pending: &Pending, reason: &str) {
        if self.named.insert(pending.reference.name()) {
            let (_, call) = self.line(pending.caller, pending.call);
            report::warning(format_args!(
                "{}: `{}`: {reason}",
                self.origin(pending),
                call.uses
            ));
        }
    }

    /// Records a request that got no usable answer. After a rate limit or a rejected token no
    /// request is sent, so the rest are counted under that one failure.
    fn fail(&mut self, pending: &Pending, error: &ApiError) {
        if !self.named.insert(pending.reference.name()) {
            return;
        }
        let (_, call) = self.line(pending.caller, pending.call);
        let uses = call.uses.clone();
        if error.rate_limited || error.status == Some(401) {
            self.halted.get_or_insert_with(|| error.clone());
            self.unread += 1;
        } else {
            let message = format!(
                "{}: `{uses}` was not checked: {error}",
                self.origin(pending)
            );
            report::warning(format_args!("{message}, so the scan is incomplete"));
            self.errors.push(RunError::Skipped(message));
        }
    }

    /// One report per node, with the shortest chain from every scanned `uses:` line that reaches
    /// it; findings that every such line ignores are dropped.
    fn reports(mut self, settings: &Settings) -> Vec<FileReport> {
        let mut chains: Vec<Vec<(usize, usize, Vec<Link>)>> = vec![Vec::new(); self.nodes.len()];
        for edge in &self.edges {
            let Caller::Root(root) = edge.caller else {
                continue;
            };
            let first = self.link(edge.caller, edge.call);
            for (node, chain) in self.shortest(edge.target, first) {
                chains[node].push((root, edge.call, chain));
            }
        }
        let nodes = std::mem::take(&mut self.nodes);
        let mut files = Vec::with_capacity(nodes.len());
        for (node, chains) in nodes.into_iter().zip(chains) {
            let mut inspection = node.inspection;
            let mut through = Vec::new();
            let mut kept = Vec::new();
            for finding in inspection.findings {
                let reporting: Vec<usize> = chains
                    .iter()
                    .enumerate()
                    .filter(|(_, (root, call, _))| !self.ignores(settings, *root, *call, &finding))
                    .map(|(index, _)| index)
                    .collect();
                if !reporting.is_empty() {
                    through.push(reporting);
                    kept.push(finding);
                }
            }
            inspection.findings = kept;
            files.push(FileReport {
                path: node.path,
                base: None,
                inspection,
                unwritten: None,
                reached: Some(Reached {
                    chains: chains.into_iter().map(|(_, _, chain)| chain).collect(),
                    through,
                }),
            });
        }
        files
    }

    /// Whether the scanned `uses:` line `call` of `root` ignores `finding`: an ignore comment on
    /// the line, or a config `ignore` entry for its file and line.
    fn ignores(&self, settings: &Settings, root: usize, call: usize, finding: &Finding) -> bool {
        let file = self.roots[root];
        let line = &file.inspection.calls[call];
        // A remote input's root action is named `owner/repo@sha:action.yml`.
        let name = file
            .path
            .to_str()
            .and_then(|path| path.rsplit(['/', ':']).next())
            .unwrap_or_default();
        settings.ignores
            && (line.ignores.iter().any(|id| finding.answers_to(id))
                || settings.config.ignores_at(finding, name, line.span[0]))
    }

    /// The shortest chain from `first`, a scanned `uses:` line that leads to `start`, to every
    /// node it reaches, in breadth-first order.
    fn shortest(&self, start: usize, first: Link) -> Vec<(usize, Vec<Link>)> {
        let mut found: Vec<(usize, Vec<Link>)> = vec![(start, vec![first])];
        let mut seen = HashSet::from([start]);
        let mut queue = VecDeque::from([0]);
        while let Some(at) = queue.pop_front() {
            let node = found[at].0;
            for edge in &self.edges {
                if !matches!(edge.caller, Caller::Node(caller) if caller == node)
                    || !seen.insert(edge.target)
                {
                    continue;
                }
                let mut chain = found[at].1.clone();
                chain.push(self.link(edge.caller, edge.call));
                found.push((edge.target, chain));
                queue.push_back(found.len() - 1);
            }
        }
        found
    }

    fn link(&self, caller: Caller, call: usize) -> Link {
        let (path, call) = self.line(caller, call);
        let [line, column, end_line, end_column] = call.span;
        Link {
            path: path.to_path_buf(),
            line,
            column,
            end_line,
            end_column,
            uses: call.uses.clone(),
        }
    }
}
