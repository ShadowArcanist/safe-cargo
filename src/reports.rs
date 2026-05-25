use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Verdict produced by the analysis workflow for a single crate release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Verdict {
    Pass,
    Warn,
    Fail,
}

/// A single rule that fired during analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggeredRule {
    pub id: String,
    pub tier: u32,
    pub points: u32,
    pub detail: String,
}

/// Diff-level delta between the analyzed version and recent prior versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    pub compared_versions: Vec<String>,
    pub lines_added: u64,
    pub lines_removed: u64,
    pub new_files: Vec<String>,
    pub build_rs_changed: bool,
    pub deps_added: Vec<String>,
    pub deps_removed: Vec<String>,
}

/// Upstream metadata about the crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateMetadata {
    pub owners: Vec<String>,
    pub repository: Option<String>,
    pub downloads: u64,
}

/// Full analysis report for a single crate version.
///
/// Produced by the GitHub Actions workflow and consumed locally by the policy
/// engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub crate_name: String,
    pub version: String,
    pub published_at: Option<DateTime<Utc>>,
    pub analyzed_at: DateTime<Utc>,
    pub release_age_days: u64,
    pub score: u32,
    pub verdict: Verdict,
    pub triggered_rules: Vec<TriggeredRule>,
    #[serde(default)]
    pub rules_executed: Vec<String>,
    pub delta: Delta,
    pub metadata: CrateMetadata,
}

/// A single dependency entry inside a manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestDependency {
    pub name: String,
    pub version: String,
}

/// Manifest produced per workflow run — lists the requested crate and all of
/// its transitive dependencies that were analyzed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub requested_crate: String,
    pub requested_version: String,
    pub analyzed_at: DateTime<Utc>,
    pub dependencies: Vec<ManifestDependency>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_serializes_uppercase() {
        assert_eq!(serde_json::to_string(&Verdict::Pass).unwrap(), "\"PASS\"");
        assert_eq!(serde_json::to_string(&Verdict::Warn).unwrap(), "\"WARN\"");
        assert_eq!(serde_json::to_string(&Verdict::Fail).unwrap(), "\"FAIL\"");
    }

    #[test]
    fn verdict_deserializes_uppercase() {
        let v: Verdict = serde_json::from_str("\"PASS\"").unwrap();
        assert_eq!(v, Verdict::Pass);
    }

    #[test]
    fn round_trip_report() {
        let json = r#"{
            "crate_name": "mio",
            "version": "0.8.11",
            "published_at": "2026-05-10T12:00:00Z",
            "analyzed_at": "2026-05-25T14:30:00Z",
            "release_age_days": 15,
            "score": 12,
            "verdict": "PASS",
            "triggered_rules": [
                {"id": "build_rs_present", "tier": 3, "points": 1, "detail": "build.rs exists (unchanged from 0.8.10)"}
            ],
            "delta": {
                "compared_versions": ["0.8.10", "0.8.9", "0.8.8"],
                "lines_added": 142,
                "lines_removed": 38,
                "new_files": ["src/poll.rs"],
                "build_rs_changed": false,
                "deps_added": ["log"],
                "deps_removed": []
            },
            "metadata": {
                "owners": ["carllerche", "tokio-rs"],
                "repository": "https://github.com/tokio-rs/mio",
                "downloads": 89000000
            }
        }"#;

        let report: Report = serde_json::from_str(json).unwrap();
        assert_eq!(report.crate_name, "mio");
        assert_eq!(report.score, 12);
        assert_eq!(report.verdict, Verdict::Pass);
        assert_eq!(report.delta.lines_added, 142);
        assert_eq!(report.metadata.owners.len(), 2);

        // Ensure re-serialization round-trips cleanly.
        let serialized = serde_json::to_string(&report).unwrap();
        let report2: Report = serde_json::from_str(&serialized).unwrap();
        assert_eq!(report.crate_name, report2.crate_name);
        assert_eq!(report.score, report2.score);
    }

    #[test]
    fn round_trip_manifest() {
        let json = r#"{
            "requested_crate": "tokio",
            "requested_version": "1.38.0",
            "analyzed_at": "2026-05-25T14:30:00Z",
            "dependencies": [
                {"name": "tokio", "version": "1.38.0"},
                {"name": "mio", "version": "0.8.11"}
            ]
        }"#;

        let manifest: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.requested_crate, "tokio");
        assert_eq!(manifest.dependencies.len(), 2);

        let serialized = serde_json::to_string(&manifest).unwrap();
        let manifest2: Manifest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(manifest.requested_crate, manifest2.requested_crate);
    }
}
