#!/usr/bin/env bash
set -euo pipefail

# Tier 1 (25 points): Detect reading of sensitive environment variables
# Flags: SSH, AWS, TOKEN, SECRET, GITHUB_TOKEN, CARGO_REGISTRY_TOKEN, HOME (in build.rs)

SOURCE_DIR="${1:?Usage: env_exfil.sh <source_dir>}"

SENSITIVE_PATTERNS='env::var\("SSH|env::var\("AWS_|env::var\("TOKEN|env::var\("SECRET|env::var\("GITHUB_TOKEN|env::var\("CARGO_REGISTRY_TOKEN|env::var_os\("SSH|env::var_os\("AWS_|env::var_os\("TOKEN|env::var_os\("SECRET|env::var_os\("GITHUB_TOKEN|env::var_os\("CARGO_REGISTRY_TOKEN'

# Check all source files for sensitive env var reads
MATCHES=$(rg -n "$SENSITIVE_PATTERNS" "$SOURCE_DIR" --type rust 2>/dev/null || true)

# Also check for HOME access specifically in build.rs
BUILD_RS="${SOURCE_DIR}/build.rs"
HOME_MATCHES=""
if [ -f "$BUILD_RS" ]; then
  HOME_MATCHES=$(rg -n 'env::var\("HOME"\)|env::var\("USERPROFILE"\)|env::var_os\("HOME"\)|env::var_os\("USERPROFILE"\)' "$BUILD_RS" 2>/dev/null || true)
fi

FINDINGS=""

if [ -n "$MATCHES" ]; then
  COUNT=$(echo "$MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$MATCHES" | head -1 | cut -c1-150)
  FINDINGS="${FINDINGS}reads sensitive env vars (${COUNT} matches). First: ${FIRST}; "
fi

if [ -n "$HOME_MATCHES" ]; then
  COUNT=$(echo "$HOME_MATCHES" | wc -l | tr -d ' ')
  FINDINGS="${FINDINGS}build.rs reads HOME/USERPROFILE (${COUNT} matches); "
fi

if [ -n "$FINDINGS" ]; then
  jq -n -c \
    --arg id "env_exfil" \
    --argjson tier 1 \
    --argjson points 25 \
    --arg detail "Potential environment variable exfiltration: ${FINDINGS}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
