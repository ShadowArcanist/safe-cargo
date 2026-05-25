#!/usr/bin/env bash
set -euo pipefail

# Tier 1 (25 points): Detect bulk environment variable access
# env::vars() / env::vars_os() iterate ALL env vars — potential credential exfiltration

SOURCE_DIR="${1:?Usage: env_vars_bulk.sh <source_dir>}"

MATCHES=$(rg -n 'env::vars\b|env::vars_os\b|option_env!' "$SOURCE_DIR" --type rust 2>/dev/null || true)

if [ -n "$MATCHES" ]; then
  COUNT=$(echo "$MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$MATCHES" | head -1 | cut -c1-150)
  jq -n -c \
    --arg id "env_vars_bulk" \
    --argjson tier 1 \
    --argjson points 25 \
    --arg detail "Bulk environment variable access detected (${COUNT} matches). This can exfiltrate all credentials. First: ${FIRST}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
