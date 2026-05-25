#!/usr/bin/env bash
set -euo pipefail

# Tier 2 (10 points): Detect crate owner changes via crates.io API

CRATE_NAME="${1:?Usage: new_maintainer.sh <crate_name> <version> <prev_versions_dir>}"
VERSION="${2:?Missing version}"
PREV_VERSIONS_DIR="${3:?Missing prev_versions_dir}"

# Fetch current owners
CURRENT_OWNERS=$(curl -sf "https://crates.io/api/v1/crates/${CRATE_NAME}/owners" \
  -H "User-Agent: safe-cargo-analyzer" 2>/dev/null || echo '{"users":[]}')
CURRENT=$(echo "$CURRENT_OWNERS" | jq -r '[.users[]?.login] | sort | .[]' 2>/dev/null || echo "")

[ -z "$CURRENT" ] && exit 0

# Check if there's a previous report in the repo with owner data
PREV_REPORT=""
if [ -d "reports/${CRATE_NAME}" ]; then
  PREV_REPORT=$(ls -1 "reports/${CRATE_NAME}/"*.json 2>/dev/null | head -1 || true)
fi

if [ -n "$PREV_REPORT" ]; then
  PREV=$(jq -r '.metadata.owners // [] | sort | .[]' "$PREV_REPORT" 2>/dev/null || echo "")

  if [ -n "$PREV" ]; then
    NEW_OWNERS=$(comm -23 <(echo "$CURRENT") <(echo "$PREV") 2>/dev/null || echo "")
    REMOVED_OWNERS=$(comm -13 <(echo "$CURRENT") <(echo "$PREV") 2>/dev/null || echo "")

    if [ -n "$NEW_OWNERS" ] || [ -n "$REMOVED_OWNERS" ]; then
      DETAIL="Owner change detected."
      [ -n "$NEW_OWNERS" ] && DETAIL="$DETAIL Added: $(echo "$NEW_OWNERS" | tr '\n' ', ')."
      [ -n "$REMOVED_OWNERS" ] && DETAIL="$DETAIL Removed: $(echo "$REMOVED_OWNERS" | tr '\n' ', ')."

      jq -n -c \
        --arg id "new_maintainer" \
        --argjson tier 2 \
        --argjson points 10 \
        --arg detail "$DETAIL" \
        '{id: $id, tier: $tier, points: $points, detail: $detail}'
    fi
  fi
fi
