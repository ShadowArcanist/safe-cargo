#!/usr/bin/env bash
set -euo pipefail

# Tier 3 (3 points): Count unsafe blocks and compare with previous version

SOURCE_DIR="${1:?Usage: unsafe_blocks.sh <source_dir> [prev_versions_dir]}"
PREV_VERSIONS_DIR="${2:-}"

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

# Count unsafe blocks in current version
CURRENT_COUNT=$(rg -c 'unsafe\s*\{' "$SOURCE_DIR" --type rust 2>/dev/null | \
  awk -F: '{sum += $NF} END {print sum+0}' || echo "0")

PREV_COUNT=""
PREV_VERSION=""

if [ -n "$PREV_VERSIONS_DIR" ] && [ -d "$PREV_VERSIONS_DIR" ]; then
  LATEST_PREV=$(ls -1 "$PREV_VERSIONS_DIR" 2>/dev/null | sort -V | tail -1 || true)
  if [ -n "$LATEST_PREV" ] && [ -d "${PREV_VERSIONS_DIR}/${LATEST_PREV}" ]; then
    PREV_VERSION="$LATEST_PREV"
    PREV_ROOT=$(package_root "${PREV_VERSIONS_DIR}/${LATEST_PREV}")
    PREV_COUNT=$(rg -c 'unsafe\s*\{' "$PREV_ROOT" --type rust 2>/dev/null | \
      awk -F: '{sum += $NF} END {print sum+0}' || echo "0")
  fi
fi

# Flag if unsafe blocks increased significantly
if [ -n "$PREV_COUNT" ] && [ "$CURRENT_COUNT" -gt "$PREV_COUNT" ]; then
  INCREASE=$((CURRENT_COUNT - PREV_COUNT))
  jq -n -c \
    --arg id "unsafe_blocks" \
    --argjson tier 3 \
    --argjson points 3 \
    --arg detail "unsafe blocks increased by ${INCREASE} (${PREV_COUNT} -> ${CURRENT_COUNT}) vs previous version ${PREV_VERSION}." \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
elif [ "$CURRENT_COUNT" -gt 0 ] && [ -z "$PREV_COUNT" ]; then
  # No previous version to compare, just note the count (informational, still 1pt)
  jq -n -c \
    --arg id "unsafe_blocks" \
    --argjson tier 3 \
    --argjson points 3 \
    --arg detail "Contains ${CURRENT_COUNT} unsafe block(s). No previous version to compare." \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
