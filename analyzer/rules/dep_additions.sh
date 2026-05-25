#!/usr/bin/env bash
set -euo pipefail

# Tier 3 (3 points): Detect new dependencies added vs previous version

SOURCE_DIR="${1:?Usage: dep_additions.sh <source_dir> <prev_versions_dir>}"
PREV_VERSIONS_DIR="${2:?Missing prev_versions_dir}"

[ -d "$PREV_VERSIONS_DIR" ] || exit 0

package_root() {
  local dir="$1"
  if [ -f "${dir}/Cargo.toml" ]; then
    printf '%s\n' "$dir"
    return
  fi
  local cargo_toml
  cargo_toml=$(find "$dir" -maxdepth 2 -name "Cargo.toml" -print -quit 2>/dev/null || true)
  if [ -n "$cargo_toml" ]; then
    dirname "$cargo_toml"
  else
    printf '%s\n' "$dir"
  fi
}

LATEST_PREV=$(ls -1 "$PREV_VERSIONS_DIR" 2>/dev/null | sort -V | tail -1 || true)
[ -z "$LATEST_PREV" ] && exit 0
PREV_ROOT=$(package_root "${PREV_VERSIONS_DIR}/${LATEST_PREV}")
[ -f "${SOURCE_DIR}/Cargo.toml" ] || exit 0
[ -f "${PREV_ROOT}/Cargo.toml" ] || exit 0

# Use Python for TOML-aware dependency extraction
NEW_DEPS=$(python3 -c "
import tomllib, sys, json

def get_deps(path):
    try:
        with open(path, 'rb') as f:
            data = tomllib.load(f)
        deps = set(data.get('dependencies', {}).keys())
        deps.update(data.get('build-dependencies', {}).keys())
        deps.update(data.get('dev-dependencies', {}).keys())
        return deps
    except Exception:
        return set()

current = get_deps(sys.argv[1])
previous = get_deps(sys.argv[2])
added = sorted(current - previous)
if added:
    print(json.dumps(added))
" "${SOURCE_DIR}/Cargo.toml" "${PREV_ROOT}/Cargo.toml" 2>/dev/null || echo "")

if [ -n "$NEW_DEPS" ] && [ "$NEW_DEPS" != "null" ]; then
  COUNT=$(echo "$NEW_DEPS" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
  if [ "$COUNT" -gt 0 ]; then
    jq -n -c \
      --arg id "dep_additions" \
      --argjson tier 3 \
      --argjson points 3 \
      --arg detail "New dependencies added vs previous version: ${NEW_DEPS}" \
      '{id: $id, tier: $tier, points: $points, detail: $detail}'
  fi
fi
