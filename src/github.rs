use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

type Answer = Result<Option<Value>, ApiError>;
type Slot = Arc<Mutex<Option<Answer>>>;

/// A GitHub API request that got no usable answer.
#[derive(Clone, Debug)]
pub struct ApiError {
    /// The HTTP status, or `None` when no response arrived.
    pub status: Option<u16>,
    /// The primary or secondary rate limit refused the request.
    pub rate_limited: bool,
    pub message: String,
}

impl ApiError {
    /// The token may not read the resource: a 403 that is not a rate limit, or a 404 where the
    /// resource must exist (GitHub answers 404 for private repositories the token cannot see).
    pub fn denied(&self) -> bool {
        matches!(self.status, Some(403 | 404)) && !self.rate_limited
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl From<ApiError> for String {
    fn from(error: ApiError) -> Self {
        error.message
    }
}

/// Read-only GitHub REST client for online rules. Answers are cached per path, failures included:
/// fix validation re-runs every rule on each patched workflow and must see the same answers, and a
/// failing path is not requested again for every workflow. Workflows are inspected in parallel, so
/// each path has its own lock: concurrent callers wait for one request instead of repeating it.
pub struct GitHub {
    api: String,
    token: String,
    agent: ureq::Agent,
    cache: Mutex<HashMap<String, Slot>>,
    /// The rate-limit or rejected-token (401) failure that stops every later request: retrying
    /// before the reset only spends more of the limit, and a bad token fails every path alike.
    halted: Mutex<Option<ApiError>>,
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
            halted: Mutex::default(),
        }
    }

    /// GETs an API path such as `/repos/actions/checkout`; `None` when it does not exist.
    pub fn get(&self, path: &str) -> Answer {
        let slot = Arc::clone(
            self.cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(path.to_owned())
                .or_default(),
        );
        let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = &*slot {
            return cached.clone();
        }
        let answer = self.request(path);
        *slot = Some(answer.clone());
        answer
    }

    /// Distinct failures of cached requests, in path order.
    pub fn failures(&self) -> Vec<ApiError> {
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let mut paths: Vec<&String> = cache.keys().collect();
        paths.sort();
        let mut failures: Vec<ApiError> = Vec::new();
        for path in paths {
            if let Some(Err(error)) = &*cache[path].lock().unwrap_or_else(PoisonError::into_inner)
                && !failures.iter().any(|seen| seen.message == error.message)
            {
                failures.push(error.clone());
            }
        }
        failures
    }

    /// GETs `path` without the cache, for a path read once per run.
    pub fn request(&self, path: &str) -> Answer {
        if let Some(halted) = &*self.halted.lock().unwrap_or_else(PoisonError::into_inner) {
            return Err(halted.clone());
        }
        let failed = |status, rate_limited, detail: String| ApiError {
            status,
            rate_limited,
            message: format!("GitHub API {path}: {detail}"),
        };
        let mut response = self
            .agent
            .get(format!("{}{path}", self.api))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", concat!("crosgha/", env!("CARGO_PKG_VERSION")))
            .call()
            .map_err(|error| failed(None, false, error.to_string()))?;
        let status = response.status().as_u16();
        match status {
            200 => {
                return response
                    .body_mut()
                    .read_json::<Value>()
                    .map(Some)
                    .map_err(|error| failed(Some(status), false, error.to_string()));
            }
            404 => return Ok(None),
            _ => {}
        }
        // https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api#exceeding-the-rate-limit
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<u64>().ok())
        };
        let retry_after = header("retry-after");
        let reset =
            header("x-ratelimit-reset").filter(|_| header("x-ratelimit-remaining") == Some(0));
        let reason = response
            .body_mut()
            .read_json::<Value>()
            .ok()
            .and_then(|body| Some(body.get("message")?.as_str()?.to_owned()));
        let rate_limited = status == 429
            || (status == 403
                && (retry_after.is_some()
                    || reset.is_some()
                    || reason
                        .as_ref()
                        .is_some_and(|reason| reason.to_ascii_lowercase().contains("rate limit"))));
        let mut detail = format!("HTTP {status}");
        if let Some(reason) = reason {
            detail.push_str(&format!(" ({reason})"));
        }
        if let Some(seconds) = retry_after {
            detail.push_str(&format!("; retry after {seconds} s"));
        } else if let Some(reset) = reset {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |now| now.as_secs());
            detail.push_str(&format!(
                "; the rate limit resets in {} s",
                reset.saturating_sub(now)
            ));
        }
        let error = failed(Some(status), rate_limited, detail);
        if rate_limited || status == 401 {
            // Requests already in flight report the first failure, so it is named once.
            let mut halted = self.halted.lock().unwrap_or_else(PoisonError::into_inner);
            return Err(halted.get_or_insert(error).clone());
        }
        Err(error)
    }
}
