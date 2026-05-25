use crate::config::PolicyConfig;
use crate::error::{Error, Result};
use crate::reports::{Report, Verdict};

/// The outcome of evaluating a single report against a policy.
#[derive(Debug, Clone)]
pub struct PolicyResult {
    pub verdict: Verdict,
    pub reasons: Vec<String>,
}

/// Parse a human-friendly duration string into whole days.
///
/// Supported suffixes:
/// - `d` — days   (e.g. "3d" → 3)
/// - `w` — weeks  (e.g. "1w" → 7)
/// - `h` — hours  (e.g. "12h" → 0, rounds down)
///
/// Returns an error for unrecognised suffixes or invalid numbers.
pub fn parse_duration(s: &str) -> Result<u64> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Error::ConfigParse {
            path: "<inline>".into(),
            reason: "empty duration string".into(),
        });
    }

    let (num_str, suffix) = s.split_at(s.len() - 1);
    let value: u64 = num_str.parse().map_err(|_| Error::ConfigParse {
        path: "<inline>".into(),
        reason: format!("invalid duration number in '{s}'"),
    })?;

    match suffix {
        "d" => Ok(value),
        "w" => Ok(value.saturating_mul(7)),
        "h" => Ok(value / 24),
        other => Err(Error::ConfigParse {
            path: "<inline>".into(),
            reason: format!("unknown duration suffix '{other}' in '{s}'"),
        }),
    }
}

/// Evaluate a report against the given policy, returning all triggered reasons.
pub fn evaluate(report: &Report, policy: &PolicyConfig) -> Result<PolicyResult> {
    let mut reasons: Vec<String> = Vec::new();

    // --- score check ---
    if report.score > policy.max_score {
        reasons.push(format!(
            "risk score {} exceeds maximum allowed {}",
            report.score, policy.max_score,
        ));
    }

    // --- release age check ---
    let min_age_days = parse_duration(&policy.min_release_age)?;
    if report.release_age_days < min_age_days {
        reasons.push(format!(
            "release age {} day(s) is below minimum {} day(s)",
            report.release_age_days, min_age_days,
        ));
    }

    // Report verdict acts as a floor — if the analyzer said FAIL, we FAIL
    if report.verdict == Verdict::Fail && !reasons.iter().any(|r| r.contains("analyzer verdict")) {
        reasons.push("analyzer verdict is FAIL".to_string());
    }

    // Track warn-level signals separately
    let mut warnings: Vec<String> = Vec::new();
    if report.verdict == Verdict::Warn {
        warnings.push(format!("Report has WARN verdict (score: {})", report.score));
    }

    let has_hard_failures = !reasons.is_empty();

    let verdict = if has_hard_failures {
        Verdict::Fail
    } else if !warnings.is_empty() {
        Verdict::Warn
    } else {
        Verdict::Pass
    };

    // Merge warnings into reasons so callers can display them
    reasons.extend(warnings);

    Ok(PolicyResult { verdict, reasons })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PolicyConfig;
    use crate::reports::{CrateMetadata, Delta, Report, Verdict};
    use chrono::Utc;

    fn sample_report(score: u32, release_age_days: u64) -> Report {
        Report {
            crate_name: "test-crate".into(),
            version: "1.0.0".into(),
            published_at: Some(Utc::now()),
            analyzed_at: Utc::now(),
            release_age_days,
            score,
            verdict: Verdict::Pass,
            triggered_rules: vec![],
            delta: Delta {
                compared_versions: vec![],
                lines_added: 0,
                lines_removed: 0,
                new_files: vec![],
                build_rs_changed: false,
                deps_added: vec![],
                deps_removed: vec![],
            },
            rules_executed: vec![],
            metadata: CrateMetadata {
                owners: vec![],
                repository: None,
                downloads: 0,
            },
        }
    }

    fn default_policy() -> PolicyConfig {
        PolicyConfig {
            min_release_age: "3d".into(),
            max_score: 40,
            require_report: true,
        }
    }

    #[test]
    fn parse_duration_days() {
        assert_eq!(parse_duration("3d").unwrap(), 3);
        assert_eq!(parse_duration("0d").unwrap(), 0);
        assert_eq!(parse_duration("30d").unwrap(), 30);
    }

    #[test]
    fn parse_duration_weeks() {
        assert_eq!(parse_duration("1w").unwrap(), 7);
        assert_eq!(parse_duration("2w").unwrap(), 14);
    }

    #[test]
    fn parse_duration_hours() {
        assert_eq!(parse_duration("12h").unwrap(), 0);
        assert_eq!(parse_duration("48h").unwrap(), 2);
        assert_eq!(parse_duration("72h").unwrap(), 3);
    }

    #[test]
    fn parse_duration_invalid_suffix() {
        assert!(parse_duration("3x").is_err());
    }

    #[test]
    fn parse_duration_invalid_number() {
        assert!(parse_duration("abcd").is_err());
    }

    #[test]
    fn parse_duration_empty() {
        assert!(parse_duration("").is_err());
    }

    #[test]
    fn evaluate_pass() {
        let report = sample_report(10, 5);
        let policy = default_policy();
        let result = evaluate(&report, &policy).unwrap();
        assert_eq!(result.verdict, Verdict::Pass);
        assert!(result.reasons.is_empty());
    }

    #[test]
    fn evaluate_fail_score() {
        let report = sample_report(50, 45);
        let policy = default_policy();
        let result = evaluate(&report, &policy).unwrap();
        assert_eq!(result.verdict, Verdict::Fail);
        assert!(result.reasons[0].contains("risk score"));
    }

    #[test]
    fn evaluate_fail_age() {
        let report = sample_report(5, 1);
        let policy = default_policy();
        let result = evaluate(&report, &policy).unwrap();
        assert_eq!(result.verdict, Verdict::Fail);
        assert!(result.reasons[0].contains("release age"));
    }

    #[test]
    fn evaluate_fail_both() {
        let report = sample_report(50, 1);
        let policy = default_policy();
        let result = evaluate(&report, &policy).unwrap();
        assert_eq!(result.verdict, Verdict::Fail);
        assert_eq!(result.reasons.len(), 2);
    }

    #[test]
    fn evaluate_fail_when_report_verdict_is_fail_even_with_low_score() {
        let mut report = sample_report(0, 30);
        report.verdict = Verdict::Fail;
        let policy = default_policy();
        let result = evaluate(&report, &policy).unwrap();
        assert_eq!(result.verdict, Verdict::Fail);
        assert!(result
            .reasons
            .iter()
            .any(|r| r.contains("analyzer verdict")));
    }
}
