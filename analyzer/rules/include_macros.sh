#!/usr/bin/env bash
set -euo pipefail

# Detect Rust include macros.
#
# include! and #[path] can substitute Rust code at compile time, so they are
# review-worthy Tier 2 signals. include_str!/include_bytes! embed data and are
# common in benign crates, so they are Tier 3 informational notes.

SOURCE_DIR="${1:?Usage: include_macros.sh <source_dir>}"

CODE_MATCHES=$(rg -n 'include!\s*\(|core::include!\s*\(|#\[path\s*=' "$SOURCE_DIR" --type rust 2>/dev/null || true)
DATA_MATCHES=$(rg -n 'include_bytes!\s*\(|include_str!\s*\(|core::include_bytes!\s*\(|core::include_str!\s*\(' "$SOURCE_DIR" --type rust 2>/dev/null || true)

if [ -n "$CODE_MATCHES" ]; then
  COUNT=$(echo "$CODE_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$CODE_MATCHES" | head -1 | cut -c1-150)
  jq -n -c \
    --arg id "include_code_macros" \
    --argjson tier 2 \
    --argjson points 10 \
    --arg detail "Uses include! macros or #[path =] overrides (${COUNT} occurrences). These can substitute Rust code at compile time. First: ${FIRST}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi

if [ -n "$DATA_MATCHES" ]; then
  COUNT=$(echo "$DATA_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$DATA_MATCHES" | head -1 | cut -c1-150)
  jq -n -c \
    --arg id "include_data_macros" \
    --argjson tier 3 \
    --argjson points 3 \
    --arg detail "Uses include_str!/include_bytes! macros (${COUNT} occurrences). These embed non-Rust data at compile time and should be reviewed if unexpected. First: ${FIRST}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
