use std::io::Read;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::validate;

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

/// Top-level response from `GET /api/v1/crates/{name}`.
#[derive(Debug, Deserialize)]
struct CrateResponse {
    versions: Vec<CrateVersion>,
}

/// A single version entry returned inside the crate response.
#[derive(Debug, Clone, Deserialize)]
pub struct CrateVersion {
    pub num: String,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
    pub yanked: bool,
}

/// Public crate info returned by [`CratesIoClient::fetch_crate_info`].
#[derive(Debug)]
pub struct CrateInfo {
    pub versions: Vec<CrateVersion>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

const BASE_URL: &str = "https://crates.io/api/v1";
const USER_AGENT: &str = "safe-cargo/0.1.0 (supply-chain-security-tool)";
const RATE_LIMIT: Duration = Duration::from_secs(1);
const MAX_RESPONSE_SIZE: usize = 2 * 1024 * 1024; // 2MB

/// HTTP client for the crates.io registry API.
///
/// Enforces crates.io's rate-limit policy (max 1 request per second) and
/// sends the mandatory `User-Agent` header on every request.
pub struct CratesIoClient {
    agent: ureq::Agent,
    /// Timestamp of the last request, used to throttle.
    last_request: Option<Instant>,
}

impl CratesIoClient {
    /// Create a new client with the required User-Agent header.
    pub fn new() -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();

        Self {
            agent,
            last_request: None,
        }
    }

    // -- Rate limiting -----------------------------------------------------

    /// Sleep if necessary to respect the 1 req/sec rate limit, then update the
    /// last-request timestamp.
    fn throttle(&mut self) {
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed();
            if elapsed < RATE_LIMIT {
                thread::sleep(RATE_LIMIT - elapsed);
            }
        }
        self.last_request = Some(Instant::now());
    }

    // -- Internal HTTP helpers ---------------------------------------------

    /// Perform a GET request, respecting rate limits, and deserialise the JSON
    /// response into `T`.
    fn get_json<T: serde::de::DeserializeOwned>(&mut self, url: &str) -> Result<T> {
        self.throttle();

        let response = self
            .agent
            .get(url)
            .header("User-Agent", USER_AGENT)
            .call()
            .map_err(map_ureq_error)?;

        let mut body_str = String::new();
        response
            .into_body()
            .as_reader()
            .take((MAX_RESPONSE_SIZE + 1) as u64)
            .read_to_string(&mut body_str)
            .map_err(|e| Error::CratesIoApi(format!("failed to read response: {e}")))?;

        if body_str.len() > MAX_RESPONSE_SIZE {
            return Err(Error::ResponseTooLarge(url.to_string(), MAX_RESPONSE_SIZE));
        }

        let parsed: T = serde_json::from_str(&body_str)
            .map_err(|e| Error::CratesIoApi(format!("failed to parse response JSON: {e}")))?;

        Ok(parsed)
    }

    // -- Public API --------------------------------------------------------

    /// Fetch crate metadata including every published version.
    pub fn fetch_crate_info(&mut self, name: &str) -> Result<CrateInfo> {
        validate::crate_name(name)?;
        let url = format!("{BASE_URL}/crates/{name}");
        let resp: CrateResponse = self.get_json(&url)?;

        Ok(CrateInfo {
            versions: resp.versions,
        })
    }

    /// Resolve a semver version requirement to the latest matching non-yanked
    /// version.
    ///
    /// If `req` is `None`, returns the latest non-yanked version.
    ///
    /// The requirement string follows Cargo's semver syntax (e.g. `"^1.2"`,
    /// `">=0.3, <0.5"`).
    pub fn resolve_version(&mut self, name: &str, req: Option<&str>) -> Result<Version> {
        validate::crate_name(name)?;
        let info = self.fetch_crate_info(name)?;

        // Parse the requirement (if any).
        let version_req = match req {
            Some(r) => {
                let parsed = VersionReq::parse(r).map_err(|e| {
                    Error::CratesIoApi(format!(
                        "invalid version requirement '{r}' for crate '{name}': {e}"
                    ))
                })?;
                Some(parsed)
            }
            None => None,
        };

        // Collect all non-yanked versions, parsed into semver::Version.
        let mut candidates: Vec<Version> = Vec::new();

        for v in &info.versions {
            if v.yanked {
                continue;
            }

            let parsed = Version::parse(&v.num).map_err(|e| {
                Error::CratesIoApi(format!(
                    "crates.io returned unparseable version '{}' for crate '{name}': {e}",
                    v.num
                ))
            })?;

            let matches = match &version_req {
                Some(vr) => vr.matches(&parsed),
                None => true,
            };

            if matches {
                candidates.push(parsed);
            }
        }

        // Sort descending and pick the latest.
        candidates.sort_by(|a, b| b.cmp(a));

        candidates.into_iter().next().ok_or_else(|| {
            let detail = match req {
                Some(r) => format!("no non-yanked version of '{name}' matches requirement '{r}'"),
                None => format!("no non-yanked versions found for crate '{name}'"),
            };
            Error::CratesIoApi(detail)
        })
    }
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

/// Convert a `ureq::Error` into our `Error` type.
fn map_ureq_error(err: ureq::Error) -> Error {
    match err {
        ureq::Error::StatusCode(404) => Error::CratesIoApi("not found (404)".to_string()),
        ureq::Error::StatusCode(429) => {
            Error::CratesIoApi("rate limited by crates.io (429)".to_string())
        }
        ureq::Error::StatusCode(code) => Error::CratesIoApi(format!("HTTP {code}")),
        _ => Error::Network(format!("{err}")),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the User-Agent is set (cannot directly inspect the agent,
    /// but we can at least ensure the constructor does not panic).
    #[test]
    fn client_creates_without_panic() {
        let _client = CratesIoClient::new();
    }

    /// Verify that version requirement parsing works for valid inputs.
    #[test]
    fn parse_version_req_valid() {
        let req = VersionReq::parse("^1.2").expect("should parse");
        assert!(req.matches(&Version::new(1, 3, 0)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
    }

    /// Verify that version requirement parsing rejects garbage.
    #[test]
    fn parse_version_req_invalid() {
        assert!(VersionReq::parse("not-a-version").is_err());
    }
}
