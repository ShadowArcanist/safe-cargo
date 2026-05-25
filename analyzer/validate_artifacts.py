#!/usr/bin/env python3
"""Validate analyzer artifacts before they are signed."""

import json
import pathlib
import sys


def fail(message: str) -> None:
    print(f"artifact validation failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def load_json(path: pathlib.Path):
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot read {path}: {exc}")


def package_set_from_metadata(metadata: dict) -> set[tuple[str, str]]:
    packages = set()
    for package in metadata.get("packages", []):
        name = package.get("name")
        version = package.get("version")
        if name == "dep-resolver":
            continue
        if not name or not version:
            fail("metadata package entry missing name or version")
        packages.add((name, version))
    return packages


def dependency_set_from_manifest(manifest: dict) -> set[tuple[str, str]]:
    deps = set()
    for dep in manifest.get("dependencies", []):
        name = dep.get("name")
        version = dep.get("version")
        if not name or not version:
            fail("manifest dependency entry missing name or version")
        entry = (name, version)
        if entry in deps:
            fail(f"duplicate manifest dependency: {name}@{version}")
        deps.add(entry)
    return deps


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: validate_artifacts.py <reports_dir> <deps.json>", file=sys.stderr)
        return 2

    reports_dir = pathlib.Path(sys.argv[1])
    deps_path = pathlib.Path(sys.argv[2])
    manifest_dir = reports_dir / "_manifests"

    manifests = sorted(manifest_dir.glob("*.json"))
    if len(manifests) != 1:
        fail(f"expected exactly one manifest, found {len(manifests)}")

    metadata = load_json(deps_path)
    manifest = load_json(manifests[0])

    expected = package_set_from_metadata(metadata)
    actual = dependency_set_from_manifest(manifest)
    if expected != actual:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        fail(f"manifest dependency mismatch; missing={missing}, extra={extra}")

    for name, version in sorted(actual):
        report_path = reports_dir / name / f"{version}.json"
        report = load_json(report_path)
        if report.get("crate_name") != name or report.get("version") != version:
            fail(f"report identity mismatch for {name}@{version}")
        if "score" not in report or "verdict" not in report:
            fail(f"report missing score or verdict for {name}@{version}")

    missing_deps = pathlib.Path("missing_deps.txt")
    if missing_deps.exists():
        for line in missing_deps.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            if "@" not in line:
                fail(f"malformed missing dependency entry: {line}")
            name, version = line.rsplit("@", 1)
            if (name, version) not in actual:
                fail(f"missing dependency not present in manifest: {line}")

    print(f"validated {len(actual)} reports against {manifests[0]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
