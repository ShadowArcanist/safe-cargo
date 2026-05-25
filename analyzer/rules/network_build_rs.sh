#!/usr/bin/env bash
set -euo pipefail

# Tier 1 (25 points): Detect network-related imports/calls in build.rs
# Flags: reqwest, ureq, hyper, curl, TcpStream, UdpSocket, std::net::, tokio::net::

SOURCE_DIR="${1:?Usage: network_build_rs.sh <source_dir>}"

BUILD_RS="${SOURCE_DIR}/build.rs"

# If no build.rs, nothing to check
[ -f "$BUILD_RS" ] || exit 0

PATTERNS=(
  "reqwest"
  "ureq"
  "hyper"
  "curl"
  "TcpStream"
  "UdpSocket"
  "std::net::"
  "tokio::net::"
)

PATTERN=$(IFS="|"; echo "${PATTERNS[*]}")

MATCHES=$(rg -n "$PATTERN" "$BUILD_RS" 2>/dev/null || true)

if [ -n "$MATCHES" ]; then
  # Summarize findings
  MATCH_COUNT=$(echo "$MATCHES" | wc -l | tr -d ' ')
  FIRST_MATCH=$(echo "$MATCHES" | head -1)
  jq -n -c \
    --arg id "network_build_rs" \
    --argjson tier 1 \
    --argjson points 25 \
    --arg detail "build.rs contains network-related code (${MATCH_COUNT} matches). First: ${FIRST_MATCH}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
