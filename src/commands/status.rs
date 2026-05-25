use colored::Colorize;

use crate::cache;
use crate::cargo::parse_cargo_lock;
use crate::error::Result;
use crate::reports::Verdict;

/// Show analysis status for all dependencies (informational only).
///
/// Unlike `check`, this command never returns an error for missing or failing
/// reports — it simply displays the current state.
pub fn run() -> Result<()> {
    let packages = parse_cargo_lock()?;

    if packages.is_empty() {
        println!(
            "{}",
            "No crates.io dependencies found in Cargo.lock.".dimmed()
        );
        return Ok(());
    }

    eprintln!(
        "{}",
        "Note: status shows cached data. Run `safe-cargo check` for verified results.".dimmed()
    );
    println!(
        "Status for {} dependencies:\n",
        packages.len().to_string().bold()
    );

    let mut analyzed: usize = 0;
    let pending: usize = 0;
    let mut missing_count: usize = 0;

    for pkg in &packages {
        let label = format!("{}@{}", pkg.name, pkg.version);

        match cache::get_report(&pkg.name, &pkg.version)? {
            Some(report) if report.crate_name != pkg.name || report.version != pkg.version => {
                // Cached report does not match expected name/version — treat as missing
                println!(
                    "  {} {:<40} {:>8}",
                    "!".red(),
                    label,
                    "integrity mismatch".red(),
                );
                missing_count += 1;
            }
            Some(report) => {
                let status_icon = match report.verdict {
                    Verdict::Pass => "✓".green(),
                    Verdict::Warn => "⚠".yellow(),
                    Verdict::Fail => "✗".red(),
                };

                let verdict_text = match report.verdict {
                    Verdict::Pass => "analyzed".green(),
                    Verdict::Warn => "warning".yellow(),
                    Verdict::Fail => "failed".red(),
                };

                println!(
                    "  {} {:<40} {:>8} (score: {})",
                    status_icon, label, verdict_text, report.score,
                );

                match report.verdict {
                    Verdict::Pass | Verdict::Warn => {
                        analyzed += 1;
                    }
                    Verdict::Fail => {
                        // Count as analyzed but failing — still "analyzed"
                        // in the sense that a report exists.
                        analyzed += 1;
                    }
                }
            }
            None => {
                // No cached report — check if we should call it "pending"
                // (i.e. analysis may have been triggered) or "missing".
                // Without network access here, we conservatively call it
                // "missing".  The user can run `safe-cargo check` for a
                // full refresh from GitHub.
                println!("  {} {:<40} {:>8}", "✗".yellow(), label, "missing".yellow(),);
                missing_count += 1;
            }
        }
    }

    // We track "pending" separately if we ever add logic to detect
    // in-flight workflow runs.  For now it stays at 0.
    let _ = pending;

    // -- Summary -------------------------------------------------------------
    println!();
    println!("{}", "─".repeat(60));
    println!(
        "  {} analyzed, {} pending, {} missing",
        analyzed.to_string().green().bold(),
        pending.to_string().yellow().bold(),
        missing_count.to_string().yellow().bold(),
    );
    println!("{}", "─".repeat(60));

    if missing_count > 0 {
        println!();
        println!(
            "{} Run {} to fetch reports, or {} to trigger analysis.",
            "hint:".yellow().bold(),
            "safe-cargo check".bold(),
            "safe-cargo audit".bold(),
        );
    }

    Ok(())
}
