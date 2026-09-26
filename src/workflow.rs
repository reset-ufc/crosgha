use yaml_serde::Value;
use yamlpath::{Document, FeatureKind, Route};

pub struct Workflow {
    pub document: Document,
    pub value: Value,
    pub jobs_start: (usize, usize),
}

impl Workflow {
    pub fn parse(source: String) -> Result<Self, String> {
        let document = Document::new(source).map_err(|error| error.to_string())?;
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
        let (line, column) = jobs.location.point_span.0;
        let jobs_start = (line + 1, column + 1);
        let value = yaml_serde::from_str(document.source()).map_err(|error| error.to_string())?;
        Ok(Self {
            document,
            value,
            jobs_start,
        })
    }
}
