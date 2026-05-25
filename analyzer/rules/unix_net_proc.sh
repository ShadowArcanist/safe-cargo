#!/usr/bin/env bash
set -euo pipefail

# Tier 2 (10 points): Detect Linux/Unix-specific attack vectors
# Flags: Unix domain sockets, /proc access, cargo:rustc-env injection

SOURCE_DIR="${1:?Usage: unix_net_proc.sh <source_dir>}"

FINDINGS=""

# Detect Unix domain socket usage
UNIX_NET_MATCHES=$(rg -n 'std::os::unix::net' "$SOURCE_DIR" --type rust 2>/dev/null || true)

if [ -n "$UNIX_NET_MATCHES" ]; then
  COUNT=$(echo "$UNIX_NET_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$UNIX_NET_MATCHES" | head -1 | cut -c1-120)
  FINDINGS="${FINDINGS}Unix domain sockets (${COUNT} matches, first: ${FIRST}); "
fi

# Detect /proc/self access patterns
PROC_SELF_MATCHES=$(rg -n '/proc/self/environ|/proc/self/fd/|/proc/self/mem' "$SOURCE_DIR" --type rust 2>/dev/null || true)

if [ -n "$PROC_SELF_MATCHES" ]; then
  COUNT=$(echo "$PROC_SELF_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$PROC_SELF_MATCHES" | head -1 | cut -c1-120)
  FINDINGS="${FINDINGS}/proc/self access (${COUNT} matches, first: ${FIRST}); "
fi

# Detect cargo:rustc-env in build.rs (env var injection via build scripts)
BUILD_RS="${SOURCE_DIR}/build.rs"
if [ -f "$BUILD_RS" ]; then
  RUSTC_ENV_MATCHES=$(rg -n 'cargo:rustc-env' "$BUILD_RS" 2>/dev/null || true)
  if [ -n "$RUSTC_ENV_MATCHES" ]; then
    COUNT=$(echo "$RUSTC_ENV_MATCHES" | wc -l | tr -d ' ')
    FIRST=$(echo "$RUSTC_ENV_MATCHES" | head -1 | cut -c1-120)
    FINDINGS="${FINDINGS}cargo:rustc-env in build.rs (${COUNT} matches, first: ${FIRST}); "
  fi
fi

if [ -n "$FINDINGS" ]; then
  jq -n -c \
    --arg id "unix_net_proc" \
    --argjson tier 2 \
    --argjson points 10 \
    --arg detail "Unix/Linux-specific attack vectors detected: ${FINDINGS}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
