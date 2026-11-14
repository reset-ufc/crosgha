//! Workflow traversal shared by the rules: jobs, steps, triggers, and `uses` refs, plus the fixes
//! that add a step input or remove a key.

use yaml_serde::{Mapping, Value};
use yamlpatch::{Op, Patch};
use yamlpath::Route;

use super::{ChangeKind, Edit, Finding, ProposedFix, Rule, online};
use crate::workflow::{Kind, Workflow};

/// A workflow job, or the `runs` of a composite action, whose steps run inside the caller's job.
pub struct Job<'a> {
    pub kind: Kind,
    /// The job ID; `runs` for a composite action.
    pub name: &'a str,
    pub value: &'a Value,
    pub route: Route<'static>,
}

impl<'a> Job<'a> {
    pub fn location(&self) -> String {
        match self.kind {
            Kind::Workflow => format!("job `{}`", self.name),
            Kind::Action => "action".to_owned(),
        }
    }

    /// What a message calls the owner of the steps.
    pub fn noun(&self) -> &'static str {
        match self.kind {
            Kind::Workflow => "job",
            Kind::Action => "action",
        }
    }

    pub fn get(&self, key: &str) -> Option<&'a Value> {
        self.value.get(key)
    }
}

/// The jobs of a workflow, or the one step list of a composite action. Rules registered as
/// reading steps (`Reads::Steps` in `rules/mod.rs`) see an action through this; a JavaScript or
/// Docker action has no steps, so it has no job. `runs` holds none of a job's keys (`runs-on`,
/// `env`, `if`, ...), so checks of those find nothing there.
pub fn jobs(workflow: &Workflow) -> Vec<Job<'_>> {
    if workflow.kind == Kind::Action {
        let runs = Route::default().with_key("runs");
        return match workflow.value.get("runs") {
            Some(value) if workflow.using().as_deref() == Some("composite") => vec![Job {
                kind: Kind::Action,
                name: "runs",
                value,
                route: runs,
            }],
            _ => Vec::new(),
        };
    }
    let Some(jobs) = workflow.value.get("jobs").and_then(Value::as_mapping) else {
        return Vec::new();
    };
    jobs.iter()
        .filter_map(|(name, value)| {
            let name = name.as_str()?;
            Some(Job {
                kind: Kind::Workflow,
                name,
                value,
                route: Route::default().with_key("jobs").with_key(name.to_owned()),
            })
        })
        .collect()
}

pub struct Step<'a> {
    pub value: &'a Value,
    pub route: Route<'static>,
    pub location: String,
}

impl<'a> Step<'a> {
    pub fn uses(&self) -> Option<&'a str> {
        self.value.get("uses").and_then(Value::as_str)
    }

    pub fn run(&self) -> Option<&'a str> {
        self.value.get("run").and_then(Value::as_str)
    }

    pub fn input(&self, key: &str) -> Option<&'a Value> {
        self.value.get("with").and_then(|with| with.get(key))
    }

    pub fn input_str(&self, key: &str) -> Option<&'a str> {
        self.input(key).and_then(Value::as_str)
    }

    pub fn key(&self, key: &str) -> Route<'static> {
        self.route.with_key(key.to_owned())
    }

    /// Route of the `uses` or `run` key, the line that identifies the step.
    pub fn anchor(&self) -> Route<'static> {
        if self.value.get("uses").is_some() {
            self.key("uses")
        } else if self.value.get("run").is_some() {
            self.key("run")
        } else {
            self.route.clone()
        }
    }

    /// Whether the step uses `action` (`owner/repo[/path]`) at any ref, ignoring case.
    pub fn uses_action(&self, action: &str) -> bool {
        self.uses()
            .and_then(|uses| uses.split_once('@'))
            .is_some_and(|(name, _)| name.eq_ignore_ascii_case(action))
    }
}

/// A fix that sets the step input `key` to `value`, adding a `with` mapping when the step has none.
pub fn add_input(step: &Step<'_>, key: &str, value: Value) -> ProposedFix {
    let (patch, changed_route) = match step.value.get("with") {
        None => {
            let mut inputs = Mapping::new();
            inputs.insert(Value::String(key.to_owned()), value);
            (
                Patch {
                    route: step.route.clone(),
                    operation: Op::Add {
                        key: "with".to_owned(),
                        value: Value::Mapping(inputs),
                    },
                },
                step.key("with"),
            )
        }
        Some(_) => {
            let with = step.key("with");
            (
                Patch {
                    route: with.clone(),
                    operation: Op::Add {
                        key: key.to_owned(),
                        value,
                    },
                },
                with.with_key(key.to_owned()),
            )
        }
    };
    ProposedFix {
        edit: Edit::Patch(patch),
        changed_route,
        change: ChangeKind::Add,
    }
}

/// A fix that removes the key at `route`. yamlpath also removes the block mappings above it that
/// the removal leaves empty.
pub fn remove_key(route: Route<'static>) -> ProposedFix {
    ProposedFix {
        edit: Edit::Patch(Patch {
            route: route.clone(),
            operation: Op::Remove,
        }),
        changed_route: route,
        change: ChangeKind::Remove,
    }
}

/// The source of the value at `route`, from its block header or quote to its last character, when
/// only the colon and comments stand between it and its key. An alias resolves to its anchor
/// elsewhere in the file, and an anchor or a tag sits in between; a rewrite of such a value would
/// change every alias of it.
pub fn own_scalar<'a>(workflow: &'a Workflow, route: &Route) -> Option<&'a str> {
    let document = &workflow.document;
    let key = document.query_key_only(route).ok()?;
    let node = document.query_exact(route).ok()??;
    let between: String = document
        .source()
        .get(key.location.byte_span.1..node.location.byte_span.0)?
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default())
        .collect();
    (between.trim() == ":").then(|| document.extract(&node))
}

/// Every step of a job, including steps nested in `parallel` groups.
pub fn steps<'a>(job: &Job<'a>) -> Vec<Step<'a>> {
    let mut found = Vec::new();
    let Some(steps) = job.value.get("steps").and_then(Value::as_sequence) else {
        return found;
    };
    for (index, value) in steps.iter().enumerate() {
        let route = job.route.with_key("steps").with_key(index);
        let location = match job.kind {
            Kind::Workflow => format!("job `{}`, step {}", job.name, index + 1),
            Kind::Action => format!("step {}", index + 1),
        };
        if let Some(group) = value.get("parallel").and_then(Value::as_sequence) {
            for (inner, nested) in group.iter().enumerate() {
                found.push(Step {
                    value: nested,
                    route: route.with_key("parallel").with_key(inner),
                    location: format!("{location}.{}", inner + 1),
                });
            }
        }
        found.push(Step {
            value,
            route,
            location,
        });
    }
    found
}

/// The `if` of a job or step.
pub struct Condition<'a> {
    pub value: &'a Value,
    pub route: Route<'static>,
    pub location: String,
}

/// Every job and step condition, each job's before those of its steps.
pub fn conditions(workflow: &Workflow) -> Vec<Condition<'_>> {
    let mut found = Vec::new();
    for job in jobs(workflow) {
        if let Some(value) = job.get("if") {
            found.push(Condition {
                value,
                route: job.route.with_key("if"),
                location: job.location(),
            });
        }
        for step in steps(&job) {
            if let Some(value) = step.value.get("if") {
                found.push(Condition {
                    value,
                    route: step.key("if"),
                    location: step.location,
                });
            }
        }
    }
    found
}

pub struct Trigger<'a> {
    pub name: &'a str,
    pub config: Option<&'a Value>,
    /// `on.<name>` for the mapping form, `on` for the string and list forms.
    pub route: Route<'static>,
}

pub fn triggers(workflow: &Workflow) -> Vec<Trigger<'_>> {
    let on = Route::default().with_key("on");
    match workflow.value.get("on") {
        Some(Value::String(name)) => vec![Trigger {
            name,
            config: None,
            route: on,
        }],
        Some(Value::Sequence(names)) => names
            .iter()
            .filter_map(Value::as_str)
            .map(|name| Trigger {
                name,
                config: None,
                route: on.clone(),
            })
            .collect(),
        Some(Value::Mapping(events)) => events
            .iter()
            .filter_map(|(name, config)| {
                let name = name.as_str()?;
                Some(Trigger {
                    name,
                    config: Some(config).filter(|config| !config.is_null()),
                    route: on.with_key(name.to_owned()),
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub fn has_trigger(workflow: &Workflow, names: &[&str]) -> bool {
    triggers(workflow)
        .iter()
        .any(|trigger| names.contains(&trigger.name))
}

/// Triggers that run in the base repository's privileged context with data an outsider controls:
/// https://docs.github.com/en/actions/reference/security/secure-use#mitigating-the-risks-of-untrusted-code-checkout
pub const PRIVILEGED_TRIGGERS: &[&str] = &["pull_request_target", "workflow_run"];

/// Events that anyone who can open an issue, comment, fork, or star can start; `fork` and `watch`
/// run on the default branch:
/// https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows
pub const EXTERNAL_TRIGGERS: &[&str] = &[
    "fork",
    "watch",
    "issues",
    "issue_comment",
    "discussion",
    "discussion_comment",
    "pull_request_target",
    "pull_request_review",
    "pull_request_review_comment",
    "workflow_run",
];

/// Visits every string scalar below `value`, with the route of the key that holds it.
pub fn strings<'a>(
    value: &'a Value,
    route: &Route<'static>,
    visit: &mut impl FnMut(&Route<'static>, &'a str),
) {
    match value {
        Value::String(text) => visit(route, text),
        Value::Sequence(items) => {
            for (index, item) in items.iter().enumerate() {
                strings(item, &route.with_key(index), visit);
            }
        }
        Value::Mapping(entries) => {
            for (key, item) in entries {
                if let Some(key) = key.as_str() {
                    strings(item, &route.with_key(key.to_owned()), visit);
                }
            }
        }
        _ => {}
    }
}

/// `owner/repo` (lowercased, as GitHub resolves it) and the `major.minor...` numbers of a step's
/// action, when its ref names a version.
pub fn versioned_action(workflow: &Workflow, step: &Step<'_>) -> Option<(String, Vec<u64>)> {
    let action = step.uses().and_then(parse_uses)?;
    let version = online::ref_version(workflow, &step.key("uses"), action.git_ref)?;
    let name = format!("{}/{}", action.owner, action.repo).to_ascii_lowercase();
    Some((name, online::numbers(&version)))
}

/// The major version of a step's action, when its ref names one.
pub fn action_major(workflow: &Workflow, step: &Step<'_>) -> Option<u64> {
    versioned_action(workflow, step)?.1.first().copied()
}

/// A boolean action input or variable: a YAML boolean, or `true`/`false` in any case with
/// surrounding whitespace, as actions read them (`getInput` trims).
pub fn input_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::String(text) if text.trim().eq_ignore_ascii_case("true") => Some(true),
        Value::String(text) if text.trim().eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    }
}

/// A remote `owner/repo[/path]@ref` reference from `uses`.
pub struct ActionRef<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub git_ref: &'a str,
}

pub fn parse_uses(uses: &str) -> Option<ActionRef<'_>> {
    if uses.starts_with("./")
        || uses.starts_with("$/")
        || uses.starts_with("docker://")
        || uses.contains("${{")
    {
        return None;
    }
    let (name, git_ref) = uses.rsplit_once('@')?;
    let mut parts = name.splitn(3, '/');
    let owner = parts.next().filter(|owner| !owner.is_empty())?;
    let repo = parts.next().filter(|repo| !repo.is_empty())?;
    Some(ActionRef {
        owner,
        repo,
        git_ref,
    })
}

/// Every `uses` in the workflow: job-level reusable workflow calls and steps.
pub fn all_uses(workflow: &Workflow) -> Vec<(Route<'static>, String, &str)> {
    let mut found = Vec::new();
    for job in jobs(workflow) {
        if let Some(uses) = job.get("uses").and_then(Value::as_str) {
            found.push((job.route.with_key("uses"), job.location(), uses));
        }
        for step in steps(&job) {
            if let Some(uses) = step.uses() {
                found.push((step.key("uses"), step.location.clone(), uses));
            }
        }
    }
    found
}

/// Paths of the local actions a workflow calls, such as `.github/actions/setup` for
/// `uses: ./.github/actions/setup`.
pub fn local_actions(workflow: &Workflow) -> Vec<String> {
    all_uses(workflow)
        .into_iter()
        .filter_map(|(_, _, uses)| uses.strip_prefix("./").map(str::to_owned))
        .collect()
}

/// 1-based start and end of the key a route ends with, or of the node for index routes. The
/// end column is exclusive, as SARIF's `endColumn` is.
pub fn span(workflow: &Workflow, route: &Route) -> Result<[usize; 4], String> {
    let feature = workflow
        .document
        .query_key_only(route)
        .or_else(|_| workflow.document.query_pretty(route))
        .map_err(|error| error.to_string())?;
    let ((line, column), (end_line, end_column)) = feature.location.point_span;
    Ok([line + 1, column + 1, end_line + 1, end_column + 1])
}

pub fn line(workflow: &Workflow, route: &Route) -> Result<usize, String> {
    Ok(span(workflow, route)?[0])
}

pub fn finding(
    workflow: &Workflow,
    rule: &'static Rule,
    route: Route<'static>,
    location: impl Into<String>,
    message: impl Into<String>,
) -> Result<Finding, String> {
    let [line, column, end_line, end_column] = span(workflow, &route)?;
    Ok(Finding {
        rule,
        severity: rule.severity,
        message: message.into(),
        location: location.into(),
        line,
        column,
        end_line,
        end_column,
        route,
        fix: None,
        concepts: &[],
    })
}

/// How specifically a zizmor repository pattern matches a `uses` value, or `None` when it does
/// not. Higher is more specific: `owner/repo/path@ref` > `owner/repo/path` > `owner/repo` >
/// `owner/repo/*` > `owner/*` > `*`. https://docs.zizmor.sh/configuration/#repository-patterns
pub fn pattern_specificity(pattern: &str, uses: &str) -> Option<u8> {
    let reference = parse_uses(uses)?;
    let (name, _) = uses.rsplit_once('@')?;
    let same = |left: &str, right: &str| left.eq_ignore_ascii_case(right);
    if pattern == "*" {
        return Some(0);
    }
    if pattern.contains('@') {
        return same(pattern, uses).then_some(5);
    }
    let parts: Vec<&str> = pattern.split('/').collect();
    match parts.as_slice() {
        [owner, "*"] => same(owner, reference.owner).then_some(1),
        [owner, repo, "*"] => {
            (same(owner, reference.owner) && same(repo, reference.repo)).then_some(2)
        }
        [_, _] => same(pattern, name).then_some(3),
        _ => same(pattern, name).then_some(4),
    }
}

/// The value of the most specific pattern in `patterns` that matches `uses`.
pub fn most_specific<'a, T>(
    patterns: impl IntoIterator<Item = (&'a str, T)>,
    uses: &str,
) -> Option<T> {
    patterns
        .into_iter()
        .filter_map(|(pattern, value)| Some((pattern_specificity(pattern, uses)?, value)))
        .max_by_key(|(specificity, _)| *specificity)
        .map(|(_, value)| value)
}
