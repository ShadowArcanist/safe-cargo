#!/usr/bin/env bash
set -euo pipefail

# Tier 2 (10 points): Detect network-related code in runtime source (not build.rs)

SOURCE_DIR="${1:?Usage: network_runtime.sh <source_dir>}"

# Exclude build.rs — that's covered by network_build_rs.sh (Tier 1)
MATCHES=$(rg -n 'reqwest|ureq|hyper::client|TcpStream|UdpSocket|tokio::net::' "$SOURCE_DIR" --type rust --glob '!build.rs' 2>/dev/null || true)

if [ -n "$MATCHES" ]; then
  COUNT=$(echo "$MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$MATCHES" | head -1 | cut -c1-120)
  jq -n -c \
    --arg id "network_runtime" \
    --argjson tier 2 \
    --argjson points 10 \
    --arg detail "Runtime source contains network code (${COUNT} matches, excluding build.rs). First: ${FIRST}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
