use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::reports::{Manifest, Report};

/// Write to a temp file then atomically rename to the target path.
/// This prevents TOCTOU races where a reader sees a partially-written file.
fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path has no parent directory",
        )
    })?;
    fs::create_dir_all(dir)?;
    let tmp_name = format!(
        "{}.tmp.{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    );
    let tmp_path = dir.join(tmp_name);
    fs::write(&tmp_path, contents)?;
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }
    Ok(())
}

/// Return the root cache directory (`~/.cache/safe-cargo/`), creating it and
/// its sub-directories if they do not yet exist.
pub fn cache_dir() -> Result<PathBuf> {
    let base = dirs::cache_dir()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "unable to determine user cache directory",
            )
        })?
        .join("safe-cargo");

    fs::create_dir_all(base.join("reports"))?;
    fs::create_dir_all(base.join("manifests"))?;

    Ok(base)
}

// ---------------------------------------------------------------------------
// Reports — stored at  <cache>/reports/<name>/<version>.json
// ---------------------------------------------------------------------------

/// Look up a cached report. Returns `Ok(None)` when the file does not exist.
pub fn get_report(name: &str, version: &str) -> Result<Option<Report>> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;

    let path = cache_dir()?
        .join("reports")
        .join(&name)
        .join(format!("{version_str}.json"));

    match fs::read_to_string(&path) {
        Ok(data) => {
            let report: Report =
                serde_json::from_str(&data).map_err(|e| crate::error::Error::ReportParse {
                    name: name.to_owned(),
                    reason: e.to_string(),
                })?;
            Ok(Some(report))
        }
        Err(_) => Ok(None),
    }
}

#[allow(dead_code)]
pub fn store_report(name: &str, version: &str, report: &Report) -> Result<()> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;

    let path = cache_dir()?
        .join("reports")
        .join(&name)
        .join(format!("{version_str}.json"));
    let data = serde_json::to_string(report).map_err(|e| crate::error::Error::ReportParse {
        name: name.to_owned(),
        reason: e.to_string(),
    })?;

    atomic_write(&path, &data)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Manifests — stored at  <cache>/manifests/<name>-<version>.json
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub fn get_manifest(name: &str, version: &str) -> Result<Option<Manifest>> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;

    let path = cache_dir()?
        .join("manifests")
        .join(format!("{name}-{version_str}.json"));

    match fs::read_to_string(&path) {
        Ok(data) => {
            let manifest: Manifest =
                serde_json::from_str(&data).map_err(|e| crate::error::Error::ReportParse {
                    name: name.to_owned(),
                    reason: e.to_string(),
                })?;
            Ok(Some(manifest))
        }
        Err(_) => Ok(None),
    }
}

/// Store a signature file alongside a cached report.
pub fn store_report_sig(name: &str, version: &str, sig: &str) -> Result<()> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;
    let path = cache_dir()?
        .join("reports")
        .join(&name)
        .join(format!("{version_str}.json.minisig"));
    atomic_write(&path, sig)?;
    Ok(())
}

/// Get a cached report signature.
pub fn get_report_sig(name: &str, version: &str) -> Result<Option<String>> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;
    let path = cache_dir()?
        .join("reports")
        .join(&name)
        .join(format!("{version_str}.json.minisig"));
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(Some(contents)),
        Err(_) => Ok(None),
    }
}

/// Evict a report and its signature from cache.
pub fn evict_report(name: &str, version: &str) -> Result<()> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;
    let dir = cache_dir()?.join("reports").join(&name);
    let _ = fs::remove_file(dir.join(format!("{version_str}.json")));
    let _ = fs::remove_file(dir.join(format!("{version_str}.json.minisig")));
    Ok(())
}

#[allow(dead_code)]
pub fn store_manifest(name: &str, version: &str, manifest: &Manifest) -> Result<()> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;

    let path = cache_dir()?
        .join("manifests")
        .join(format!("{name}-{version_str}.json"));
    let data = serde_json::to_string(manifest).map_err(|e| crate::error::Error::ReportParse {
        name: name.to_owned(),
        reason: e.to_string(),
    })?;

    atomic_write(&path, &data)?;
    Ok(())
}

/// Store a signature file alongside a cached manifest.
pub fn store_manifest_sig(name: &str, version: &str, sig: &str) -> Result<()> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;
    let path = cache_dir()?
        .join("manifests")
        .join(format!("{name}-{version_str}.json.minisig"));
    atomic_write(&path, sig)?;
    Ok(())
}

/// Store a report as raw JSON (original bytes, not re-serialized).
pub fn store_report_raw(name: &str, version: &str, raw_json: &str) -> Result<()> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;

    let path = cache_dir()?
        .join("reports")
        .join(&name)
        .join(format!("{version_str}.json"));
    atomic_write(&path, raw_json)?;
    Ok(())
}

/// Get a cached report as raw JSON string (original bytes, not deserialized).
pub fn get_report_raw(name: &str, version: &str) -> Result<Option<String>> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;

    let path = cache_dir()?
        .join("reports")
        .join(&name)
        .join(format!("{version_str}.json"));

    match fs::read_to_string(&path) {
        Ok(contents) => Ok(Some(contents)),
        Err(_) => Ok(None),
    }
}

/// Store a manifest as raw JSON (original bytes, not re-serialized).
pub fn store_manifest_raw(name: &str, version: &str, raw_json: &str) -> Result<()> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;

    let path = cache_dir()?
        .join("manifests")
        .join(format!("{name}-{version_str}.json"));
    atomic_write(&path, raw_json)?;
    Ok(())
}

/// Get a cached manifest as raw JSON string (original bytes, not deserialized).
pub fn get_manifest_raw(name: &str, version: &str) -> Result<Option<String>> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;

    let path = cache_dir()?
        .join("manifests")
        .join(format!("{name}-{version_str}.json"));

    match fs::read_to_string(&path) {
        Ok(contents) => Ok(Some(contents)),
        Err(_) => Ok(None),
    }
}

/// Get a cached manifest signature.
pub fn get_manifest_sig(name: &str, version: &str) -> Result<Option<String>> {
    let name = crate::validate::safe_path_component(name)?;
    let version_str = crate::validate::safe_path_component(version)?;
    let path = cache_dir()?
        .join("manifests")
        .join(format!("{name}-{version_str}.json.minisig"));
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(Some(contents)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::{CrateMetadata, Delta, Manifest, ManifestDependency, Report, Verdict};
    use chrono::Utc;

    fn sample_report() -> Report {
        Report {
            crate_name: "test-crate".into(),
            version: "0.1.0".into(),
            published_at: Some(Utc::now()),
            analyzed_at: Utc::now(),
            release_age_days: 10,
            score: 2,
            verdict: Verdict::Pass,
            triggered_rules: vec![],
            rules_executed: vec![],
            delta: Delta {
                compared_versions: vec![],
                lines_added: 0,
                lines_removed: 0,
                new_files: vec![],
                build_rs_changed: false,
                deps_added: vec![],
                deps_removed: vec![],
            },
            metadata: CrateMetadata {
                owners: vec!["alice".into()],
                repository: Some("https://github.com/alice/test-crate".into()),
                downloads: 1000,
            },
        }
    }

    fn sample_manifest() -> Manifest {
        Manifest {
            requested_crate: "test-crate".into(),
            requested_version: "0.1.0".into(),
            analyzed_at: Utc::now(),
            dependencies: vec![ManifestDependency {
                name: "test-crate".into(),
                version: "0.1.0".into(),
            }],
        }
    }

    #[test]
    fn cache_dir_creates_structure() {
        let dir = cache_dir().unwrap();
        assert!(dir.join("reports").is_dir());
        assert!(dir.join("manifests").is_dir());
    }

    #[test]
    fn report_round_trip() {
        let report = sample_report();
        store_report("roundtrip-test", "0.1.0", &report).unwrap();

        let loaded = get_report("roundtrip-test", "0.1.0").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.crate_name, "test-crate");
        assert_eq!(loaded.score, 2);

        // Clean up.
        let _ = std::fs::remove_dir_all(cache_dir().unwrap().join("reports/roundtrip-test"));
    }

    #[test]
    fn get_report_missing_returns_none() {
        let result = get_report("nonexistent-crate", "99.99.99").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn manifest_round_trip() {
        let manifest = sample_manifest();
        store_manifest("manifest-test", "0.1.0", &manifest).unwrap();

        let loaded = get_manifest("manifest-test", "0.1.0").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.requested_crate, "test-crate");

        // Clean up.
        let _ = std::fs::remove_file(
            cache_dir()
                .unwrap()
                .join("manifests/manifest-test-0.1.0.json"),
        );
    }

    #[test]
    fn get_manifest_missing_returns_none() {
        let result = get_manifest("nonexistent-crate", "99.99.99").unwrap();
        assert!(result.is_none());
    }
}
