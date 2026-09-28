use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use serde_json::Value;

type Slot = Arc<Mutex<Option<Option<Value>>>>;

/// Read-only GitHub REST client for online rules. Responses are cached per path because fix
/// validation re-runs every rule on each patched workflow. Workflows are inspected in parallel,
/// so each path has its own lock: concurrent callers wait for one request instead of repeating it.
pub struct GitHub {
    api: String,
    token: String,
    agent: ureq::Agent,
    cache: Mutex<HashMap<String, Slot>>,
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
            cache: Mutex::default(),
        }
    }

    /// GETs an API path such as `/repos/actions/checkout`; `None` when it does not exist.
    pub fn get(&self, path: &str) -> Result<Option<Value>, String> {
        let slot = Arc::clone(
            self.cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(path.to_owned())
                .or_default(),
        );
        let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = &*slot {
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
        // Errors stay uncached, so the next caller retries.
        *slot = Some(value.clone());
        Ok(value)
    }
}
