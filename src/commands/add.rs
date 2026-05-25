use colored::Colorize;

use crate::cache;
use crate::cargo;
use crate::cli;
use crate::config::SafeCargoConfig;
use crate::error::{Error, Result};
use crate::github::GitHubClient;
use crate::keychain;
use crate::policy;
use crate::reports::{Manifest, Report, Verdict};
use crate::resolver::CratesIoClient;
use crate::signing;

/// Add a crate after verifying that it (and all of its transitive
/// dependencies) pass the configured security policy.
///
/// `crate_spec` is either a bare crate name (`"tokio"`) or a pinned spec
/// (`"tokio@1.38.0"`).
pub fn run(crate_spec: &str) -> Result<()> {
    // -- 1. Load config & credentials ----------------------------------------
    let config = SafeCargoConfig::load()?;
    let repo = &config.registry.repo;
    let branch = &config.registry.branch;
    let token = keychain::get_token(repo)?;

    if config.registry.signing_key.is_none() {
        return Err(Error::SigningRequired);
    }

    // -- 2. Parse the crate spec ---------------------------------------------
    let (crate_name, requested_version) = cli::parse_crate_spec(crate_spec);

    crate::validate::crate_name(&crate_name)?;
    if let Some(ref v) = requested_version {
        crate::validate::version(v)?;
    }

    // -- 3. Resolve version via crates.io ------------------------------------
    let mut crates_io = CratesIoClient::new();
    let resolved = crates_io.resolve_version(&crate_name, requested_version.as_deref())?;
    let version = resolved.to_string();

    println!(
        "Resolving {} {} ...",
        crate_name.bold(),
        format!("v{version}").dimmed(),
    );

    // -- 4. Create GitHub client ---------------------------------------------
    let github = GitHubClient::new(repo, branch, &token);
    let signing_key = config.registry.signing_key.as_deref();

    // -- 5. Fetch manifest (local cache, then GitHub) ------------------------
    let manifest = fetch_manifest_cached(&github, &crate_name, &version, signing_key)?;

    let manifest = match manifest {
        Some(m) => m,
        None => {
            // No manifest at all — trigger analysis and exit.
            return handle_no_manifest(&github, &crate_name, &version);
        }
    };

    // Verify manifest matches our request
    if manifest.requested_crate != crate_name || manifest.requested_version != version {
        eprintln!(
            "{}",
            format!(
                "Warning: manifest mismatch. Expected {}@{}, got {}@{}. Re-triggering analysis.",
                crate_name, version, manifest.requested_crate, manifest.requested_version,
            )
            .yellow()
        );
        return handle_no_manifest(&github, &crate_name, &version);
    }

    // -- 6. Evaluate every dependency against policy -------------------------
    let mut passed: Vec<String> = Vec::new();
    let mut failed: Vec<(String, Vec<String>)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for dep in &manifest.dependencies {
        crate::validate::crate_name(&dep.name)?;
        crate::validate::version(&dep.version)?;

        // Allowlisted crates skip analysis entirely.
        if config.is_allowed(&dep.name, &dep.version) {
            passed.push(format!("{} v{} (allowlisted)", dep.name, dep.version));
            continue;
        }

        // Try to obtain a report: local cache first, then GitHub.
        let report = fetch_report_cached(&github, &dep.name, &dep.version, signing_key)?;

        match report {
            None => {
                missing.push(format!("{} v{}", dep.name, dep.version));
            }
            Some(report) => {
                // Verify report matches the expected dep
                if report.crate_name != dep.name || report.version != dep.version {
                    return Err(Error::ReportIntegrityFailure(
                        dep.name.clone(),
                        dep.version.clone(),
                    ));
                }

                let result = policy::evaluate(&report, &config.policy)?;
                match result.verdict {
                    Verdict::Pass => {
                        passed.push(format!("{} v{}", dep.name, dep.version));
                    }
                    Verdict::Warn => {
                        failed.push((format!("{} v{}", dep.name, dep.version), result.reasons));
                    }
                    Verdict::Fail => {
                        failed.push((format!("{} v{}", dep.name, dep.version), result.reasons));
                    }
                }
            }
        }
    }

    // -- 7. Handle missing reports -------------------------------------------
    if !missing.is_empty() {
        println!();
        println!(
            "{}",
            "Some dependencies have not been analyzed yet:"
                .yellow()
                .bold(),
        );
        for dep in &missing {
            println!("  {} {}", "?".yellow(), dep.yellow());
        }
        println!();

        // Trigger a workflow for the top-level crate so the analysis covers
        // all transitive deps.
        match github.trigger_workflow(&crate_name, &version) {
            Ok(_) => {
                println!(
                    "Analysis workflow triggered for {} v{}.",
                    crate_name.bold(),
                    version,
                );
            }
            Err(e) => {
                eprintln!(
                    "{} failed to trigger analysis workflow: {}",
                    "warning:".yellow().bold(),
                    e,
                );
            }
        }

        if let Ok(Some(pr_url)) = github.get_latest_pr(&crate_name) {
            println!("Follow progress: {}", pr_url);
        }

        println!();
        println!(
            "Run `safe-cargo add {}` again after the analysis completes.",
            crate_spec,
        );
        std::process::exit(1);
    }

    // -- 8. Handle policy failures -------------------------------------------
    if !failed.is_empty() {
        println!();
        println!(
            "{}",
            "Security policy violations detected — aborting installation."
                .red()
                .bold(),
        );
        println!();
        for (dep, reasons) in &failed {
            println!("  {} {}", "FAIL".red().bold(), dep.red());
            for reason in reasons {
                println!("       - {}", reason);
            }
        }
        println!();
        println!(
            "To override, add the crate to the [allow] table in safe-cargo.toml \
             with a reason.",
        );
        std::process::exit(1);
    }

    // -- 9. All passed — print summary and install ---------------------------
    println!();
    println!(
        "{}",
        format!("All {} dependencies passed security policy.", passed.len(),)
            .green()
            .bold(),
    );
    for dep in &passed {
        println!("  {} {}", "OK".green().bold(), dep);
    }
    println!();
    println!("Running: cargo add {}@{} ...", crate_name.bold(), version,);

    cargo::run(&["add", &format!("{crate_name}@{version}")])?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fetch a manifest, checking the local cache first and falling back to GitHub.
/// If fetched from GitHub, persist it to the local cache.
/// When `signing_key` is `Some`, signatures are required and verified against
/// the **raw** bytes (never re-serialized JSON).
fn fetch_manifest_cached(
    github: &GitHubClient,
    name: &str,
    version: &str,
    signing_key: Option<&str>,
) -> Result<Option<Manifest>> {
    // Check local cache — read raw bytes so we can verify the original JSON.
    if let Some(raw) = cache::get_manifest_raw(name, version)? {
        if let Some(key) = signing_key {
            if let Some(sig) = cache::get_manifest_sig(name, version)? {
                if signing::verify(key, raw.as_bytes(), &sig).is_ok() {
                    let manifest: Manifest =
                        serde_json::from_str(&raw).map_err(|e| Error::ReportParse {
                            name: name.to_string(),
                            reason: e.to_string(),
                        })?;
                    return Ok(Some(manifest));
                }
            }
            // No cached sig or sig was invalid — fall through to fetch from GitHub.
        } else {
            let manifest: Manifest =
                serde_json::from_str(&raw).map_err(|e| Error::ReportParse {
                    name: name.to_string(),
                    reason: e.to_string(),
                })?;
            return Ok(Some(manifest));
        }
    }

    // Fetch raw JSON from GitHub.
    let raw = github.fetch_manifest_raw(name, version)?;

    match raw {
        Some(raw_json) => {
            // Verify signature against raw bytes before deserializing.
            if let Some(key) = signing_key {
                let sig = github.fetch_manifest_signature(name, version)?;
                match sig {
                    Some(ref sig_str) => {
                        signing::verify(key, raw_json.as_bytes(), sig_str)?;
                        // Verified — cache the original raw bytes and sig.
                        cache::store_manifest_raw(name, version, &raw_json).ok();
                        cache::store_manifest_sig(name, version, sig_str).ok();
                    }
                    None => {
                        return Err(Error::ReportParse {
                            name: name.to_string(),
                            reason: "manifest exists but signature is missing".to_string(),
                        });
                    }
                }
            } else {
                cache::store_manifest_raw(name, version, &raw_json).ok();
            }

            let manifest: Manifest =
                serde_json::from_str(&raw_json).map_err(|e| Error::ReportParse {
                    name: name.to_string(),
                    reason: e.to_string(),
                })?;
            Ok(Some(manifest))
        }
        None => Ok(None),
    }
}

/// Fetch a report, checking the local cache first and falling back to GitHub.
/// If fetched from GitHub, persist it to the local cache.
/// When `signing_key` is `Some`, signatures are required and verified against
/// the **raw** bytes (never re-serialized JSON).
fn fetch_report_cached(
    github: &GitHubClient,
    name: &str,
    version: &str,
    signing_key: Option<&str>,
) -> Result<Option<Report>> {
    // Check local cache — read raw bytes so we can verify the original JSON.
    if let Some(raw) = cache::get_report_raw(name, version)? {
        if let Some(key) = signing_key {
            if let Some(sig) = cache::get_report_sig(name, version)? {
                if signing::verify(key, raw.as_bytes(), &sig).is_ok() {
                    let report: Report =
                        serde_json::from_str(&raw).map_err(|e| Error::ReportParse {
                            name: name.to_string(),
                            reason: e.to_string(),
                        })?;
                    return Ok(Some(report));
                }
            }
            // No cached sig or sig was invalid — evict and re-fetch.
            cache::evict_report(name, version)?;
        } else {
            let report: Report = serde_json::from_str(&raw).map_err(|e| Error::ReportParse {
                name: name.to_string(),
                reason: e.to_string(),
            })?;
            return Ok(Some(report));
        }
    }

    // Fetch raw JSON from GitHub.
    let raw = github.fetch_report_raw(name, version)?;

    match raw {
        Some(raw_json) => {
            // Verify signature against raw bytes before deserializing.
            if let Some(key) = signing_key {
                let sig = github.fetch_report_signature(name, version)?;
                match sig {
                    Some(ref sig_str) => {
                        signing::verify(key, raw_json.as_bytes(), sig_str)?;
                        // Verified — cache the original raw bytes and sig.
                        cache::store_report_raw(name, version, &raw_json).ok();
                        cache::store_report_sig(name, version, sig_str).ok();
                    }
                    None => {
                        return Err(Error::ReportParse {
                            name: name.to_string(),
                            reason: "report exists but signature is missing".to_string(),
                        });
                    }
                }
            } else {
                cache::store_report_raw(name, version, &raw_json).ok();
            }

            let report: Report =
                serde_json::from_str(&raw_json).map_err(|e| Error::ReportParse {
                    name: name.to_string(),
                    reason: e.to_string(),
                })?;
            Ok(Some(report))
        }
        None => Ok(None),
    }
}

/// Handle the case where no manifest exists for the requested crate/version.
/// Triggers the analysis workflow, checks for an existing PR, and exits.
fn handle_no_manifest(github: &GitHubClient, crate_name: &str, version: &str) -> Result<()> {
    println!();
    println!(
        "{}",
        format!("{crate_name} v{version} has not been analyzed yet.")
            .yellow()
            .bold(),
    );

    // Trigger analysis workflow.
    match github.trigger_workflow(crate_name, version) {
        Ok(_) => {
            println!(
                "Analysis workflow triggered for {} v{}.",
                crate_name.bold(),
                version,
            );
        }
        Err(e) => {
            eprintln!(
                "{} failed to trigger analysis workflow: {}",
                "warning:".yellow().bold(),
                e,
            );
        }
    }

    // Check for an existing PR.
    match github.get_latest_pr(crate_name) {
        Ok(Some(pr_url)) => {
            println!("Existing analysis PR: {}", pr_url);
        }
        Ok(None) => {
            println!(
                "Check your repository's Actions tab for workflow progress: \
                 https://github.com/{}/actions",
                github_repo_from_client(crate_name),
            );
        }
        Err(e) => {
            eprintln!(
                "{} failed to check for existing PR: {}",
                "warning:".yellow().bold(),
                e,
            );
        }
    }

    println!();
    println!(
        "Run `safe-cargo add {}@{}` again after the analysis completes.",
        crate_name, version,
    );
    std::process::exit(1);
}

/// Best-effort helper to provide a repo URL hint in messages. We don't have
/// direct access to the repo field on GitHubClient, so we re-read it from the
/// config. If that fails we return a placeholder.
fn github_repo_from_client(_crate_name: &str) -> String {
    SafeCargoConfig::load()
        .map(|c| c.registry.repo)
        .unwrap_or_else(|_| "<your-repo>".to_string())
}
