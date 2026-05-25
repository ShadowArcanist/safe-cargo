#!/usr/bin/env python3
"""
Filter already-analyzed dependencies from cargo metadata output.

Usage: python3 filter_missing.py deps.json reports/
Outputs: crate@version lines for deps that need analysis.
"""

import json
import os
import sys


def main():
    if len(sys.argv) < 3:
        print("Usage: filter_missing.py <deps.json> <reports_dir>", file=sys.stderr)
        sys.exit(1)

    deps_file = sys.argv[1]
    reports_dir = sys.argv[2]

    # Read cargo metadata
    try:
        with open(deps_file, "r") as f:
            metadata = json.load(f)
    except (FileNotFoundError, json.JSONDecodeError) as e:
        print(f"Error reading {deps_file}: {e}", file=sys.stderr)
        sys.exit(1)

    # Extract all resolved packages
    packages = metadata.get("packages", [])

    # Also check resolved deps from the resolve graph
    resolve = metadata.get("resolve", {})
    resolved_nodes = resolve.get("nodes", [])

    # Build a set of (name, version) from packages
    deps = set()
    for pkg in packages:
        name = pkg.get("name", "")
        version = pkg.get("version", "")
        # Skip the synthetic dep-resolver package we created
        if name == "dep-resolver":
            continue
        if name and version:
            deps.add((name, version))

    # Check which already have reports
    missing = []
    for name, version in sorted(deps):
        report_path = os.path.join(reports_dir, name, f"{version}.json")
        if os.path.isfile(report_path) and os.path.getsize(report_path) > 0:
            continue  # already analyzed
        missing.append(f"{name}@{version}")

    # Output missing deps
    for dep in missing:
        print(dep)


if __name__ == "__main__":
    main()
