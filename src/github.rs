use std::io::Read;

use crate::error::{Error, Result};
use crate::reports::{Manifest, Report};
use crate::validate;

// ---------------------------------------------------------------------------
// GitHub API response types
// ---------------------------------------------------------------------------

/// Represents a pull request from the GitHub API (only the fields we need).
#[derive(Debug, serde::Deserialize)]
struct PullRequest {
    html_url: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

const USER_AGENT: &str = "safe-cargo/0.1.0";
const MAX_RESPONSE_SIZE: usize = 2 * 1024 * 1024; // 2MB

pub struct GitHubClient {
    token: String,
    repo: String,
    branch: String,
    agent: ureq::Agent,
}

impl GitHubClient {
    /// Create a new GitHub API client.
    ///
    /// - `repo`   — owner/name, e.g. `"ShadowArcanist/safe-cargo"`
    /// - `branch` — branch where reports live, e.g. `"main"`
    /// - `token`  — GitHub personal access token (never logged)
    pub fn new(repo: &str, branch: &str, token: &str) -> Self {
        Self {
            token: token.to_string(),
            repo: repo.to_string(),
            branch: branch.to_string(),
            agent: ureq::Agent::config_builder()
                .timeout_global(Some(std::time::Duration::from_secs(30)))
                .build()
                .into(),
        }
    }

    // -- Public API --------------------------------------------------------

    /// Fetch a pre-existing analysis report for a crate version.
    ///
    /// Returns `Ok(None)` when the report file does not exist (HTTP 404).
    #[allow(dead_code)]
    pub fn fetch_report(&self, name: &str, version: &str) -> Result<Option<Report>> {
        validate::crate_name(name)?;
        validate::version(version)?;
        let path = format!("reports/{name}/{version}.json");
        let url = self.contents_url(&path);

        match self.get_raw(&url) {
            Ok(body) => {
                let report: Report =
                    serde_json::from_str(&body).map_err(|e| Error::ReportParse {
                        name: name.to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(Some(report))
            }
            Err(Error::ReportNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Fetch the dependency manifest for a crate version.
    ///
    /// Returns `Ok(None)` when the manifest file does not exist (HTTP 404).
    #[allow(dead_code)]
    pub fn fetch_manifest(&self, name: &str, version: &str) -> Result<Option<Manifest>> {
        validate::crate_name(name)?;
        validate::version(version)?;
        let path = format!("reports/_manifests/{name}-{version}.json");
        let url = self.contents_url(&path);

        match self.get_raw(&url) {
            Ok(body) => {
                let manifest: Manifest =
                    serde_json::from_str(&body).map_err(|e| Error::ReportParse {
                        name: name.to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(Some(manifest))
            }
            Err(Error::ReportNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Fetch the detached signature for a report.
    pub fn fetch_report_signature(&self, name: &str, version: &str) -> Result<Option<String>> {
        validate::crate_name(name)?;
        validate::version(version)?;
        let path = format!("reports/{name}/{version}.json.minisig");
        let url = self.contents_url(&path);
        match self.get_raw(&url) {
            Ok(body) => Ok(Some(body)),
            Err(Error::ReportNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Fetch the detached signature for a manifest.
    pub fn fetch_manifest_signature(&self, name: &str, version: &str) -> Result<Option<String>> {
        validate::crate_name(name)?;
        validate::version(version)?;
        let path = format!("reports/_manifests/{name}-{version}.json.minisig");
        let url = self.contents_url(&path);
        match self.get_raw(&url) {
            Ok(body) => Ok(Some(body)),
            Err(Error::ReportNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Fetch a pre-existing analysis report as a raw JSON string (not deserialized).
    ///
    /// Returns `Ok(None)` when the report file does not exist (HTTP 404).
    pub fn fetch_report_raw(&self, name: &str, version: &str) -> Result<Option<String>> {
        validate::crate_name(name)?;
        validate::version(version)?;
        let path = format!("reports/{name}/{version}.json");
        let url = self.contents_url(&path);

        match self.get_raw(&url) {
            Ok(body) => Ok(Some(body)),
            Err(Error::ReportNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Fetch the dependency manifest as a raw JSON string (not deserialized).
    ///
    /// Returns `Ok(None)` when the manifest file does not exist (HTTP 404).
    pub fn fetch_manifest_raw(&self, name: &str, version: &str) -> Result<Option<String>> {
        validate::crate_name(name)?;
        validate::version(version)?;
        let path = format!("reports/_manifests/{name}-{version}.json");
        let url = self.contents_url(&path);

        match self.get_raw(&url) {
            Ok(body) => Ok(Some(body)),
            Err(Error::ReportNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Trigger the analysis workflow via `workflow_dispatch`.
    ///
    /// Returns a URL pointing to the workflow dispatches endpoint (the
    /// GitHub API does not return a run ID synchronously).
    pub fn trigger_workflow(&self, crate_name: &str, version: &str) -> Result<String> {
        validate::crate_name(crate_name)?;
        validate::version(version)?;
        let url = format!(
            "https://api.github.com/repos/{}/actions/workflows/analyze.yml/dispatches",
            self.repo
        );

        let payload = serde_json::json!({
            "ref": self.branch,
            "inputs": {
                "crate": crate_name,
                "version": version,
            }
        });

        let _response = self
            .agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", USER_AGENT)
            .send_json(&payload)
            .map_err(|e| self.map_ureq_error(&url, e))?;

        Ok(url)
    }

    /// Search for an open pull request created by the analysis workflow for
    /// the given crate.
    ///
    /// Looks for PRs whose head branch matches `analyze/{crate_name}-*`.
    /// Returns `Ok(Some(html_url))` if a matching PR is found, `Ok(None)`
    /// otherwise.
    pub fn get_latest_pr(&self, crate_name: &str) -> Result<Option<String>> {
        validate::crate_name(crate_name)?;
        let url = format!(
            "https://api.github.com/repos/{repo}/pulls?state=open&head={repo}:analyze/{crate_name}-",
            repo = self.repo,
            crate_name = crate_name,
        );

        let body = self.get_json(&url)?;

        let prs: Vec<PullRequest> = serde_json::from_str(&body)
            .map_err(|e| Error::GitHubApi(format!("failed to parse PR list from {url}: {e}")))?;

        Ok(prs.into_iter().next().map(|pr| pr.html_url))
    }

    // -- Private helpers ---------------------------------------------------

    /// Build the GitHub Contents API URL for the given file path.
    fn contents_url(&self, path: &str) -> String {
        format!(
            "https://api.github.com/repos/{}/contents/{}?ref={}",
            self.repo, path, self.branch
        )
    }

    /// GET a file as raw content using the `application/vnd.github.v3.raw`
    /// accept header, which returns the file body directly (no base64
    /// wrapping).
    ///
    /// Returns `Error::ReportNotFound` on 404 so callers can distinguish
    /// "not found" from real failures.
    fn get_raw(&self, url: &str) -> Result<String> {
        let response = self
            .agent
            .get(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3.raw")
            .header("User-Agent", USER_AGENT)
            .call()
            .map_err(|e| self.map_ureq_error(url, e))?;

        let mut body = String::new();
        response
            .into_body()
            .as_reader()
            .take((MAX_RESPONSE_SIZE + 1) as u64)
            .read_to_string(&mut body)
            .map_err(|e| Error::Network(format!("failed to read response from {url}: {e}")))?;

        if body.len() > MAX_RESPONSE_SIZE {
            return Err(Error::ResponseTooLarge(url.to_string(), MAX_RESPONSE_SIZE));
        }

        Ok(body)
    }

    /// GET a JSON endpoint using the standard GitHub v3 accept header.
    fn get_json(&self, url: &str) -> Result<String> {
        let response = self
            .agent
            .get(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", USER_AGENT)
            .call()
            .map_err(|e| self.map_ureq_error(url, e))?;

        let mut body = String::new();
        response
            .into_body()
            .as_reader()
            .take((MAX_RESPONSE_SIZE + 1) as u64)
            .read_to_string(&mut body)
            .map_err(|e| Error::Network(format!("failed to read response from {url}: {e}")))?;

        if body.len() > MAX_RESPONSE_SIZE {
            return Err(Error::ResponseTooLarge(url.to_string(), MAX_RESPONSE_SIZE));
        }

        Ok(body)
    }

    /// Map a `ureq::Error` into our domain error, **never** including the
    /// token in the message.
    fn map_ureq_error(&self, url: &str, err: ureq::Error) -> Error {
        match err {
            ureq::Error::StatusCode(404) => Error::ReportNotFound {
                name: url.to_string(),
                version: String::new(),
            },
            ureq::Error::StatusCode(code) => Error::GitHubApi(format!("HTTP {code} from {url}")),
            _ => Error::Network(format!("request to {url} failed: {err}")),
        }
    }
}
