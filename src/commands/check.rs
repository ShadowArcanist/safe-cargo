use colored::Colorize;

use crate::cache;
use crate::cargo::parse_cargo_lock;
use crate::config::SafeCargoConfig;
use crate::error::{Error, Result};
use crate::github::GitHubClient;
use crate::keychain;
use crate::policy;
use crate::reports::{Report, Verdict};
use crate::signing;

/// Verify all dependencies in `Cargo.lock` have passing analysis reports.
///
/// Exits with an error if any dependency fails policy evaluation or has no
/// report available.
pub fn run() -> Result<()> {
    // -- 1. Load config & credentials ----------------------------------------
    let config = SafeCargoConfig::load()?;
    let token = keychain::get_token(&config.registry.repo)?;
    let client = GitHubClient::new(&config.registry.repo, &config.registry.branch, &token);
    let signing_key = config.registry.signing_key.as_deref();

    if signing_key.is_none() {
        return Err(Error::SigningRequired);
    }

    // -- 2. Parse Cargo.lock -------------------------------------------------
    let packages = parse_cargo_lock()?;

    if packages.is_empty() {
        println!(
            "{}",
            "No crates.io dependencies found in Cargo.lock.".dimmed()
        );
        return Ok(());
    }

    println!(
        "Checking {} dependencies...\n",
        packages.len().to_string().bold()
    );

    // -- 3. Evaluate each package --------------------------------------------
    let mut passed: Vec<String> = Vec::new();
    let mut failed: Vec<(String, Vec<String>)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for pkg in &packages {
        let label = format!("{}@{}", pkg.name, pkg.version);

        // Check allowlist first.
        if config.is_allowed(&pkg.name, &pkg.version) {
            println!("  {} {} {}", "✓".green(), label, "(allowed)".dimmed());
            passed.push(label);
            continue;
        }

        // Fetch report with signature verification (raw bytes flow).
        let report = fetch_report_cached(&client, &pkg.name, &pkg.version, signing_key)?;

        match report {
            Some(report) => {
                // Verify report name/version matches the expected package.
                if report.crate_name != pkg.name || report.version != pkg.version {
                    return Err(Error::ReportIntegrityFailure(
                        pkg.name.clone(),
                        pkg.version.clone(),
                    ));
                }

                let result = policy::evaluate(&report, &config.policy)?;
                match result.verdict {
                    Verdict::Pass => {
                        println!("  {} {} (score: {})", "✓".green(), label, report.score);
                        passed.push(label);
                    }
                    Verdict::Warn => {
                        println!(
                            "  {} {} (score: {})",
                            "\u{26a0}".yellow(),
                            label,
                            report.score
                        );
                        for reason in &result.reasons {
                            println!("    {} {}", "\u{2192}".yellow(), reason);
                        }
                        failed.push((label, result.reasons));
                    }
                    Verdict::Fail => {
                        println!("  {} {} (score: {})", "✗".red(), label.red(), report.score);
                        for reason in &result.reasons {
                            println!("    {} {}", "\u{2192}".red(), reason);
                        }
                        failed.push((label, result.reasons));
                    }
                }
            }
            None => {
                println!(
                    "  {} {} {}",
                    "?".yellow(),
                    label.yellow(),
                    "no report".dimmed()
                );
                missing.push(label);
            }
        }
    }

    // -- 4. Summary table ----------------------------------------------------
    println!();
    println!("{}", "─".repeat(50));
    println!(
        "  {} passed, {} failed, {} missing",
        passed.len().to_string().green().bold(),
        failed.len().to_string().red().bold(),
        missing.len().to_string().yellow().bold(),
    );
    println!("{}", "─".repeat(50));

    // -- 5. Exit with error if anything went wrong ---------------------------
    if !failed.is_empty() || !missing.is_empty() {
        let mut problem_crates: Vec<String> = Vec::new();
        for (label, _) in &failed {
            problem_crates.push(label.clone());
        }
        for label in &missing {
            problem_crates.push(label.clone());
        }

        if !missing.is_empty() {
            println!();
            println!(
                "{} Run {} to trigger analysis for missing crates.",
                "hint:".yellow().bold(),
                "safe-cargo audit".bold(),
            );
        }

        return Err(Error::UnanalyzedDeps {
            crates: problem_crates,
        });
    }

    println!();
    println!(
        "{}",
        "All dependencies pass security policy.".green().bold()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fetch a report, checking the local cache first and falling back to GitHub.
/// Verifies signatures against raw bytes when `signing_key` is `Some`.
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
