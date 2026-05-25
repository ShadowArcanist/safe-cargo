#!/usr/bin/env python3
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


class PrSummaryTests(unittest.TestCase):
    def test_summary_groups_review_notes_and_formats_rule_details(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = pathlib.Path(tmp)
            reports = base / "reports"
            (reports / "_manifests").mkdir(parents=True)
            (reports / "safe_dep").mkdir()
            (reports / "review_dep").mkdir()

            manifest = {
                "requested_crate": "demo",
                "requested_version": "1.0.0",
                "analyzed_at": "2026-05-25T00:00:00Z",
                "dependencies": [
                    {"name": "safe_dep", "version": "1.0.0"},
                    {"name": "review_dep", "version": "2.0.0"},
                ],
            }
            manifest_path = reports / "_manifests" / "demo-1.0.0.json"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

            common_delta = {
                "compared_versions": [],
                "lines_added": 0,
                "lines_removed": 0,
                "new_files": [],
                "build_rs_changed": False,
                "deps_added": [],
                "deps_removed": [],
            }
            (reports / "safe_dep" / "1.0.0.json").write_text(
                json.dumps(
                    {
                        "crate_name": "safe_dep",
                        "version": "1.0.0",
                        "published_at": None,
                        "analyzed_at": "2026-05-25T00:00:00Z",
                        "release_age_days": 30,
                        "score": 0,
                        "verdict": "PASS",
                        "triggered_rules": [],
                        "delta": common_delta,
                        "metadata": {
                            "owners": ["alice"],
                            "repository": "https://github.com/acme/safe_dep",
                            "downloads": 1234,
                        },
                    }
                ),
                encoding="utf-8",
            )
            (reports / "review_dep" / "2.0.0.json").write_text(
                json.dumps(
                    {
                        "crate_name": "review_dep",
                        "version": "2.0.0",
                        "published_at": None,
                        "analyzed_at": "2026-05-25T00:00:00Z",
                        "release_age_days": 1,
                        "score": 28,
                        "verdict": "WARN",
                        "triggered_rules": [
                            {
                                "id": "hardcoded_build_targets",
                                "tier": 1,
                                "points": 25,
                                "detail": "build.rs contains hardcoded network target",
                            },
                            {
                                "id": "release_age",
                                "tier": 3,
                                "points": 3,
                                "detail": "Very recent release",
                            },
                        ],
                        "delta": common_delta,
                        "metadata": {
                            "owners": ["bob"],
                            "repository": None,
                            "downloads": 55,
                        },
                    }
                ),
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "analyzer" / "pr_summary.py"),
                    str(manifest_path),
                    "demo",
                    "1.0.0",
                    str(reports),
                ],
                check=True,
                text=True,
                stdout=subprocess.PIPE,
            )

        output = result.stdout
        self.assertIn("## Analysis report for `demo@1.0.0`", output)
        self.assertIn("### Needs Review", output)
        self.assertIn("#### Risk Signals", output)
        self.assertIn("#### Informational Signals", output)
        self.assertIn("`hardcoded_build_targets`", output)
        self.assertIn("`release_age`", output)
        self.assertNotIn("<details><summary>", output)


if __name__ == "__main__":
    unittest.main()
