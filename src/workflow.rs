use std::{collections::HashMap, sync::Mutex};

use yaml_serde::Value;
use yamlpath::{Document, FeatureKind, Route};

pub const BOM: char = '\u{feff}';

pub struct Workflow {
    pub document: Document,
    pub value: Value,
    /// Version comments of `uses` lines by route, filled by `rules::online::comment_version`:
    /// several rules ask for the same line.
    pub comment_versions: Mutex<HashMap<String, Option<String>>>,
}

impl Workflow {
    /// Parses any YAML file, such as `dependabot.yml`, without workflow validation.
    pub fn parse_yaml(mut source: String) -> Result<Self, String> {
        // GitHub accepts a UTF-8 BOM, but the YAML scanner counts it as a column and then rejects
        // a key on the first line; `filesystem::write_verified` puts it back.
        if source.starts_with(BOM) {
            source.drain(..BOM.len_utf8());
        }
        let document = Document::new(source).map_err(|error| error.to_string())?;
        let value = yaml_serde::from_str(document.source()).map_err(|error| error.to_string())?;
        Ok(Self {
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
}
