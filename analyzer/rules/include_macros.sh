#!/usr/bin/env bash
set -euo pipefail

# Tier 1 (25 points): Detect include_bytes!, include_str!, include! macros
# These can load arbitrary content from non-Rust files at compile time

SOURCE_DIR="${1:?Usage: include_macros.sh <source_dir>}"

MATCHES=$(rg -n 'include_bytes!\s*\(|include_str!\s*\(|include!\s*\(|core::include_bytes!\s*\(|core::include_str!\s*\(|core::include!\s*\(|#\[path\s*=' "$SOURCE_DIR" --type rust 2>/dev/null || true)

if [ -n "$MATCHES" ]; then
  COUNT=$(echo "$MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$MATCHES" | head -1 | cut -c1-150)
  jq -n -c \
    --arg id "include_macros" \
    --argjson tier 1 \
    --argjson points 25 \
    --arg detail "Uses include_bytes!/include_str!/include! macros or #[path =] overrides (${COUNT} occurrences). These can embed arbitrary content. First: ${FIRST}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
