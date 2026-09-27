use std::{cell::RefCell, collections::HashMap, time::Duration};

use serde_json::Value;

/// Read-only GitHub REST client for online rules. Responses are cached per path because fix
/// validation re-runs every rule on each patched workflow.
pub struct GitHub {
    api: String,
    token: String,
    agent: ureq::Agent,
    cache: RefCell<HashMap<String, Option<Value>>>,
}

impl GitHub {
    pub fn new(api: &str, token: String) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        Self {
            api: api.trim_end_matches('/').to_owned(),
            token,
            agent,
            cache: RefCell::default(),
        }
    }

    /// GETs an API path such as `/repos/actions/checkout`; `None` when it does not exist.
    pub fn get(&self, path: &str) -> Result<Option<Value>, String> {
        if let Some(cached) = self.cache.borrow().get(path) {
            return Ok(cached.clone());
        }
        let mut response = self
            .agent
            .get(format!("{}{path}", self.api))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", concat!("crosgha/", env!("CARGO_PKG_VERSION")))
            .call()
            .map_err(|error| format!("GitHub API {path}: {error}"))?;
        let value = match response.status().as_u16() {
            200 => Some(
                response
                    .body_mut()
                    .read_json::<Value>()
                    .map_err(|error| format!("GitHub API {path}: {error}"))?,
            ),
            404 => None,
            status => return Err(format!("GitHub API {path}: HTTP {status}")),
        };
        self.cache
            .borrow_mut()
            .insert(path.to_owned(), value.clone());
        Ok(value)
    }
}
