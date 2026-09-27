use yaml_serde::Value;
use yamlpath::{Document, FeatureKind, Route};

pub struct Workflow {
    pub document: Document,
    pub value: Value,
}

impl Workflow {
    /// Parses any YAML file, such as `dependabot.yml`, without workflow validation.
    pub fn parse_yaml(source: String) -> Result<Self, String> {
        let document = Document::new(source).map_err(|error| error.to_string())?;
        let value = yaml_serde::from_str(document.source()).map_err(|error| error.to_string())?;
        Ok(Self { document, value })
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
