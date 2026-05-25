#!/usr/bin/env bash
set -euo pipefail

# Tier 2 (10 points): Detect if build.rs was added or changed vs previous versions
# New build.rs = high signal, changed build.rs = medium signal

SOURCE_DIR="${1:?Usage: build_rs_diff.sh <source_dir> <prev_versions_dir>}"
PREV_VERSIONS_DIR="${2:?Missing prev_versions_dir}"

BUILD_RS="${SOURCE_DIR}/build.rs"

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

# If no build.rs in current version, nothing to flag
[ -f "$BUILD_RS" ] || exit 0

# If no previous versions available, can't diff
[ -d "$PREV_VERSIONS_DIR" ] || exit 0

# Find the most recent previous version
LATEST_PREV=$(ls -1 "$PREV_VERSIONS_DIR" 2>/dev/null | sort -V | tail -1 || true)
[ -z "$LATEST_PREV" ] && exit 0
PREV_ROOT=$(package_root "${PREV_VERSIONS_DIR}/${LATEST_PREV}")

PREV_BUILD_RS="${PREV_ROOT}/build.rs"

if [ ! -f "$PREV_BUILD_RS" ]; then
  # build.rs is NEW in this version
  LINES=$(wc -l < "$BUILD_RS" | tr -d ' ')
  jq -n -c \
    --arg id "build_rs_diff" \
    --argjson tier 2 \
    --argjson points 10 \
    --arg detail "build.rs was ADDED in this version (${LINES} lines). Previous version (${LATEST_PREV}) had no build.rs." \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
else
  # Check if build.rs changed
  if ! diff -q "$BUILD_RS" "$PREV_BUILD_RS" > /dev/null 2>&1; then
    DIFF_LINES=$(diff "$BUILD_RS" "$PREV_BUILD_RS" 2>/dev/null | grep -c '^[<>]' || true)
    [ -z "$DIFF_LINES" ] && DIFF_LINES=0
    jq -n -c \
      --arg id "build_rs_diff" \
      --argjson tier 2 \
      --argjson points 10 \
      --arg detail "build.rs was MODIFIED (${DIFF_LINES} lines changed vs previous version ${LATEST_PREV})." \
      '{id: $id, tier: $tier, points: $points, detail: $detail}'
  fi
fi
