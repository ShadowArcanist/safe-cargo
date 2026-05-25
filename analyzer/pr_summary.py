#!/usr/bin/env python3
"""
Generate a PR summary table from analysis reports.

Usage: python3 pr_summary.py <manifest.json> <crate> <version> <reports_dir>
Outputs: Markdown body for the PR.
"""

import json
import os
import sys


def verdict_emoji(verdict: str) -> str:
    """Return a text indicator for the verdict."""
    if verdict == "PASS":
        return "PASS"
    elif verdict == "WARN":
        return "WARN"
    elif verdict == "FAIL":
        return "FAIL"
    return verdict


def format_age(age_days) -> str:
    """Format age in days to a human-readable string."""
    if age_days is None:
        return "?"
    age_days = int(age_days)
    if age_days == 0:
        return "<1d"
    elif age_days < 30:
        return f"{age_days}d"
    elif age_days < 365:
        months = age_days // 30
        return f"{months}mo"
    else:
        years = age_days // 365
        return f"{years}y"


def main():
    if len(sys.argv) < 5:
        print(
            "Usage: pr_summary.py <manifest.json> <crate> <version> <reports_dir>",
            file=sys.stderr,
        )
        sys.exit(1)

    manifest_file = sys.argv[1]
    crate = sys.argv[2]
    version = sys.argv[3]
    reports_dir = sys.argv[4]

    # Read manifest to get list of analyzed crates
    try:
        with open(manifest_file, "r") as f:
            manifest = json.load(f)
    except (FileNotFoundError, json.JSONDecodeError) as e:
        print(f"Error reading {manifest_file}: {e}", file=sys.stderr)
        sys.exit(1)

    # Extract the dependencies array from the manifest
    dependencies = manifest.get("dependencies", [])

    # Load full reports for each crate in the manifest
    rows = []
    fail_count = 0
    warn_count = 0
    pass_count = 0
    unknown_count = 0

    for entry in dependencies:
        dep_name = entry.get("name", "")
        dep_version = entry.get("version", "")
        if not dep_name or not dep_version:
            continue

        # Try to load the full report for richer data
        report_path = os.path.join(reports_dir, dep_name, f"{dep_version}.json")
        report = None
        try:
            with open(report_path, "r") as f:
                report = json.load(f)
        except (FileNotFoundError, json.JSONDecodeError):
            pass

        score = report.get("score", "?") if report else "?"
        verdict = report.get("verdict", "?") if report else "?"

        age_days = None
        triggered_rules = []
        if report:
            age_days = report.get("release_age_days")
            triggered_rules = report.get("triggered_rules", [])

        if verdict == "FAIL":
            fail_count += 1
        elif verdict == "WARN":
            warn_count += 1
        elif verdict == "PASS":
            pass_count += 1
        else:
            unknown_count += 1

        flags_display = ", ".join(r.get("id", "") for r in triggered_rules) if triggered_rules else "—"

        rows.append(
            {
                "crate": dep_name,
                "version": dep_version,
                "score": score,
                "age": format_age(age_days),
                "verdict": verdict_emoji(verdict),
                "flags": flags_display,
            }
        )

    # Sort: FAIL first, then WARN, then PASS; within each group sort by score desc
    verdict_order = {"FAIL": 0, "WARN": 1, "PASS": 2, "?": 3}
    rows.sort(
        key=lambda r: (
            verdict_order.get(r["verdict"], 4),
            -(r["score"] if isinstance(r["score"], (int, float)) else 0),
        )
    )

    # Build markdown
    lines = []
    lines.append(f"## Analysis: {crate}@{version}")
    lines.append("")
    summary_parts = [
        f"**{len(rows)}** dependencies analyzed:",
        f"**{fail_count}** FAIL, **{warn_count}** WARN, **{pass_count}** PASS",
    ]
    if unknown_count > 0:
        summary_parts.append(f"**{unknown_count}** unknown (no report)")
    lines.append(" ".join(summary_parts))
    lines.append("")
    lines.append("| Crate | Version | Score | Age | Verdict | Flags |")
    lines.append("|-------|---------|-------|-----|---------|-------|")

    for row in rows:
        score_str = (
            f"{row['score']}/100"
            if isinstance(row["score"], (int, float))
            else str(row["score"])
        )
        lines.append(
            f"| {row['crate']} | {row['version']} | {score_str} "
            f"| {row['age']} | {row['verdict']} | {row['flags']} |"
        )

    lines.append("")

    # Add details for any FAIL or WARN crates
    flagged = [r for r in rows if r["verdict"] in ("FAIL", "WARN")]
    if flagged:
        lines.append("### Flagged Dependencies")
        lines.append("")
        for row in flagged:
            dep_name = row["crate"]
            dep_version = row["version"]
            report_path = os.path.join(reports_dir, dep_name, f"{dep_version}.json")
            try:
                with open(report_path, "r") as f:
                    report = json.load(f)
                lines.append(f"<details><summary>{dep_name}@{dep_version} ({row['verdict']})</summary>")
                lines.append("")
                triggered_rules = report.get("triggered_rules", [])
                if triggered_rules:
                    for rule in triggered_rules:
                        tier = rule.get("tier", "?")
                        rule_id = rule.get("id", "unknown")
                        detail = rule.get("detail", "")
                        points = rule.get("points", 0)
                        lines.append(f"- **[T{tier}]** `{rule_id}` (+{points}pts): {detail}")
                else:
                    lines.append("No specific findings.")
                lines.append("")
                lines.append("</details>")
                lines.append("")
            except (FileNotFoundError, json.JSONDecodeError):
                lines.append(f"- **{dep_name}@{dep_version}**: Report not available")
                lines.append("")

    lines.append("---")
    lines.append(f"*Generated by safe-cargo analyzer*")

    print("\n".join(lines))


if __name__ == "__main__":
    main()
