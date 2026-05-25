#!/usr/bin/env bash
set -euo pipefail

# Tier 2 (10 points): Detect use-aliasing that could hide malicious imports
# Flags: aliasing of std::net, std::process, std::fs, and other security-sensitive modules

SOURCE_DIR="${1:?Usage: use_aliasing.sh <source_dir>}"

# Search for use ... as ... patterns on security-sensitive std modules
# Covers: std::net, std::process, std::fs, std::env, std::os
ALIAS_MATCHES=$(rg -n 'use\s+std::(net|process|fs|env|os)\b.*\bas\b' "$SOURCE_DIR" --type rust 2>/dev/null || true)

if [ -n "$ALIAS_MATCHES" ]; then
  COUNT=$(echo "$ALIAS_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$ALIAS_MATCHES" | head -1 | cut -c1-120)
  jq -n -c \
    --arg id "use_aliasing" \
    --argjson tier 2 \
    --argjson points 10 \
    --arg detail "Security-sensitive module aliasing detected (${COUNT} matches). Imports may be disguised. First: ${FIRST}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
