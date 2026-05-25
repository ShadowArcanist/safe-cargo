#!/usr/bin/env bash
set -euo pipefail

# Tier 3 (3 points): Flag if build.rs exists (informational)

SOURCE_DIR="${1:?Usage: build_rs_present.sh <source_dir>}"

BUILD_RS="${SOURCE_DIR}/build.rs"

if [ -f "$BUILD_RS" ]; then
  LINES=$(wc -l < "$BUILD_RS" | tr -d ' ')
  jq -n -c \
    --arg id "build_rs_present" \
    --argjson tier 3 \
    --argjson points 3 \
    --arg detail "build.rs is present (${LINES} lines). Build scripts run during compilation and have full system access." \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
