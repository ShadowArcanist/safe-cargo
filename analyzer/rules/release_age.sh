#!/usr/bin/env bash
set -euo pipefail

# Tier 3 (3 points): Flag if the release is less than 3 days old

CRATE_NAME="${1:?Usage: release_age.sh <crate_name> <version>}"
VERSION="${2:?Missing version}"

# Fetch version info from crates.io
VERSION_JSON=$(curl -sf "https://crates.io/api/v1/crates/${CRATE_NAME}/${VERSION}" \
  -H "User-Agent: safe-cargo-analyzer" 2>/dev/null || echo "{}")

PUBLISHED=$(echo "$VERSION_JSON" | jq -r '.version.created_at // empty' 2>/dev/null || echo "")

[ -z "$PUBLISHED" ] && exit 0

# Calculate age in seconds
# Handle both GNU and BSD date
PUBLISH_EPOCH=$(date -d "$PUBLISHED" +%s 2>/dev/null || \
  date -j -f "%Y-%m-%dT%H:%M:%S" "${PUBLISHED%%.*}" +%s 2>/dev/null || \
  echo "")

[ -z "$PUBLISH_EPOCH" ] && exit 0

NOW_EPOCH=$(date +%s)
AGE_SECONDS=$((NOW_EPOCH - PUBLISH_EPOCH))
AGE_DAYS=$((AGE_SECONDS / 86400))

if [ "$AGE_DAYS" -lt 3 ]; then
  if [ "$AGE_DAYS" -eq 0 ]; then
    AGE_HOURS=$((AGE_SECONDS / 3600))
    AGE_STR="${AGE_HOURS} hours"
  else
    AGE_STR="${AGE_DAYS} days"
  fi

  jq -n -c \
    --arg id "release_age" \
    --argjson tier 3 \
    --argjson points 3 \
    --arg detail "Very recent release: published ${AGE_STR} ago (${PUBLISHED}). New releases may not have been reviewed yet." \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
