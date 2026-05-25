use colored::Colorize;

use crate::cache;
use crate::cargo::{parse_cargo_lock, read_root_crate, LockPackage};
use crate::config::SafeCargoConfig;
use crate::error::{Error, Result};
use crate::github::GitHubClient;
use crate::keychain;
use crate::signing;

/// Re-trigger analysis for all dependencies that lack reports.
///
/// Reads `Cargo.lock`, identifies packages without cached reports, and
/// dispatches a single GitHub Actions workflow to analyze the dependency tree.
pub fn run() -> Result<()> {
    // -- 1. Load config & credentials ----------------------------------------
    let config = SafeCargoConfig::load()?;

    if config.registry.signing_key.is_none() {
        return Err(Error::SigningRequired);
    }

    let token = keychain::get_token(&config.registry.repo)?;
    let client = GitHubClient::new(&config.registry.repo, &config.registry.branch, &token);

    // -- 2. Parse Cargo.lock -------------------------------------------------
    let packages = parse_cargo_lock()?;

    if packages.is_empty() {
        println!(
            "{}",
            "No crates.io dependencies found in Cargo.lock.".dimmed()
        );
        return Ok(());
    }

    // -- 3. Find packages without reports ------------------------------------
    let signing_key = config.registry.signing_key.as_deref();
    let mut needs_analysis: Vec<&LockPackage> = Vec::new();
    let mut already_analyzed: usize = 0;

    for pkg in &packages {
        // Use signature-verified cache check when signing key is configured
        let has_valid_report = if let Some(key) = signing_key {
            match (
                cache::get_report_raw(&pkg.name, &pkg.version)?,
                cache::get_report_sig(&pkg.name, &pkg.version)?,
            ) {
                (Some(raw), Some(sig)) => signing::verify(key, raw.as_bytes(), &sig).is_ok(),
                _ => false,
            }
        } else {
            cache::get_report(&pkg.name, &pkg.version)?.is_some()
        };

        if has_valid_report {
            already_analyzed += 1;
        } else {
            needs_analysis.push(pkg);
        }
    }

    if needs_analysis.is_empty() {
        println!(
            "{} All {} dependencies already have reports.",
            "✓".green(),
            already_analyzed.to_string().bold(),
        );
        return Ok(());
    }

    // -- 4. Identify the root crate from Cargo.toml --------------------------
    // We trigger a single workflow dispatch using the root crate, which will
    // resolve and analyze the entire dependency tree.
    let (root_name, root_version) = read_root_crate()?;

    println!(
        "Triggering analysis for {} (root: {}@{})...\n",
        format!("{} unanalyzed dependencies", needs_analysis.len()).yellow(),
        root_name.bold(),
        root_version,
    );

    // List the missing crates for visibility.
    for pkg in &needs_analysis {
        println!("  {} {}@{}", "→".yellow(), pkg.name, pkg.version);
    }

    // -- 5. Trigger workflow -------------------------------------------------
    client.trigger_workflow(&root_name, &root_version)?;

    println!();
    println!(
        "{} Workflow dispatched for {}@{}.",
        "✓".green(),
        root_name.bold(),
        root_version,
    );
    println!("  The analysis workflow will process the full dependency tree.");
    println!(
        "  Run {} after the workflow completes to fetch results.",
        "safe-cargo check".bold(),
    );

    // -- Summary -------------------------------------------------------------
    println!();
    println!("{}", "─".repeat(50));
    println!(
        "  {} already analyzed, {} triggered for analysis",
        already_analyzed.to_string().green().bold(),
        needs_analysis.len().to_string().yellow().bold(),
    );
    println!("{}", "─".repeat(50));

    Ok(())
}
