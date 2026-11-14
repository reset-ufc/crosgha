use std::collections::HashMap;
use std::sync::Mutex;

use yaml_serde::Value;
use yamlpath::{Document, FeatureKind, Route};

/// The kind of file a scan reads. Both are parsed into a `Workflow`, so rules that read only steps
/// run on either.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Workflow,
    /// An action metadata file (`action.yml` or `action.yaml`):
    /// https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax
    Action,
}

impl Kind {
    /// What a report calls a file of this kind.
    pub fn noun(self) -> &'static str {
        match self {
            Kind::Workflow => "workflow",
            Kind::Action => "action",
        }
    }

    /// Parses `source` with this kind's structural validation.
    pub fn parse(self, source: String) -> Result<Workflow, String> {
        match self {
            Kind::Workflow => Workflow::parse(source),
            Kind::Action => Workflow::parse_action(source),
        }
    }
}

/// A parsed workflow or action metadata file.
pub struct Workflow {
    pub kind: Kind,
    pub document: Document,
    pub value: Value,
    /// Version comments of `uses` lines by route, filled by `rules::online::comment_version`:
    /// several rules ask for the same line.
    pub comment_versions: Mutex<HashMap<String, Option<String>>>,
}

impl Workflow {
    /// Parses any YAML file, such as `dependabot.yml`, without workflow validation.
    /// `source` comes from `filesystem::read`, without a BOM.
    pub fn parse_yaml(source: String) -> Result<Self, String> {
        let document = Document::new(source).map_err(|error| error.to_string())?;
        let value = yaml_serde::from_str(document.source()).map_err(|error| error.to_string())?;
        Ok(Self {
            kind: Kind::Workflow,
            document,
            value,
            comment_versions: Mutex::default(),
        })
    }

    pub fn parse(source: String) -> Result<Self, String> {
        let workflow = Self::parse_yaml(source)?;
        let document = &workflow.document;
        let root = Route::default();
        for key in ["on", "jobs"] {
            if !document.query_exists(&root.with_key(key)) {
                return Err(format!("missing required top-level key `{key}`"));
            }
        }

        let jobs = document
            .query_exact(&root.with_key("jobs"))
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "top-level `jobs` has no value".to_owned())?;
        if !matches!(
            jobs.kind(),
            FeatureKind::BlockMapping | FeatureKind::FlowMapping
        ) {
            return Err("top-level `jobs` must be a mapping".to_owned());
        }
        Ok(workflow)
    }

    /// Parses an action metadata file. Only what the runner needs to pick a runtime is checked
    /// here, a `runs` mapping with a string `using`: without it the runner refuses the action
    /// (`ActionManifestManager.cs` in actions/runner at 8cfa1049f86781d141056082d73802d56cd2b433).
    /// The rest of the metadata is checked by `schema-validity`.
    pub fn parse_action(source: String) -> Result<Self, String> {
        let mut action = Self::parse_yaml(source)?;
        action.kind = Kind::Action;
        let Some(runs) = action.value.get("runs") else {
            return Err("missing required top-level key `runs`".to_owned());
        };
        if !runs.is_mapping() {
            return Err("top-level `runs` must be a mapping".to_owned());
        }
        if !runs.get("using").is_some_and(Value::is_string) {
            return Err("`runs.using` must be a string".to_owned());
        }
        Ok(action)
    }

    /// The action's `runs.using`, lowercased: the runner compares it ignoring case.
    pub fn using(&self) -> Option<String> {
        let using = self.value.get("runs")?.get("using")?.as_str()?;
        Some(using.to_ascii_lowercase())
    }
}
