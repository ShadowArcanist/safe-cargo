#!/usr/bin/env bash
set -euo pipefail

# Detect hardcoded URLs and IP addresses.
#
# Build-time network targets are Tier 1 because build.rs runs during
# installation/compilation. Runtime literals are Tier 3 review notes: useful for
# human review, but not enough by themselves to imply a supply-chain attack.
# Flags: IP addresses, URLs not pointing to trusted domains

SOURCE_DIR="${1:?Usage: hardcoded_urls.sh <source_dir>}"

BUILD_RS="${SOURCE_DIR}/build.rs"

BUILD_FINDINGS=""
RUNTIME_FINDINGS=""

# Check build.rs (higher risk — runs at compile time)
if [ -f "$BUILD_RS" ]; then
  # Check for IPv4 addresses (excluding 127.0.0.1 and 0.0.0.0)
  IP_MATCHES=$(rg -n '[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}' "$BUILD_RS" 2>/dev/null | \
    rg -v '127\.0\.0\.1|0\.0\.0\.0' 2>/dev/null || true)

  # Check for URLs that are NOT trusted domains
  # Trusted: crates.io, github.com, docs.rs, rust-lang.org
  URL_MATCHES=$(rg -n 'https?://[^\s"'\'']+' "$BUILD_RS" 2>/dev/null | \
    rg -v 'crates\.io|github\.com|docs\.rs|rust-lang\.org|githubusercontent\.com' 2>/dev/null || true)

  if [ -n "$IP_MATCHES" ]; then
    COUNT=$(echo "$IP_MATCHES" | wc -l | tr -d ' ')
    FIRST=$(echo "$IP_MATCHES" | head -1 | cut -c1-120)
    BUILD_FINDINGS="${BUILD_FINDINGS}hardcoded IP addresses (${COUNT} matches, first: ${FIRST}); "
  fi

  if [ -n "$URL_MATCHES" ]; then
    COUNT=$(echo "$URL_MATCHES" | wc -l | tr -d ' ')
    FIRST=$(echo "$URL_MATCHES" | head -1 | cut -c1-120)
    BUILD_FINDINGS="${BUILD_FINDINGS}URLs to untrusted domains (${COUNT} matches, first: ${FIRST}); "
  fi
fi

# Secondary check: runtime source files (excluding build.rs, tests, examples)
RT_IP_MATCHES=$(rg --no-filename -n '[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}' "$SOURCE_DIR" --type rust --glob '!build.rs' 2>/dev/null | \
  rg -v '127\.0\.0\.1|0\.0\.0\.0|255\.255\.|\bversion\b|\bsemver\b' 2>/dev/null || true)

RT_URL_MATCHES=$(rg --no-filename -n 'https?://[^\s"'\'']+' "$SOURCE_DIR" --type rust --glob '!build.rs' 2>/dev/null | \
  rg -v 'crates\.io|github\.com|docs\.rs|rust-lang\.org|githubusercontent\.com|//\s*http|\bdocs?\b|\blicense\b|example\.com' 2>/dev/null || true)

if [ -n "$RT_IP_MATCHES" ]; then
  COUNT=$(echo "$RT_IP_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$RT_IP_MATCHES" | head -1 | cut -c1-120)
  RUNTIME_FINDINGS="${RUNTIME_FINDINGS}hardcoded IP addresses (${COUNT} matches, first: ${FIRST}); "
fi

if [ -n "$RT_URL_MATCHES" ]; then
  COUNT=$(echo "$RT_URL_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$RT_URL_MATCHES" | head -1 | cut -c1-120)
  RUNTIME_FINDINGS="${RUNTIME_FINDINGS}URLs to untrusted domains (${COUNT} matches, first: ${FIRST}); "
fi

if [ -n "$BUILD_FINDINGS" ]; then
  jq -n -c \
    --arg id "hardcoded_build_targets" \
    --argjson tier 1 \
    --argjson points 25 \
    --arg detail "build.rs contains hardcoded network targets: ${BUILD_FINDINGS}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi

if [ -n "$RUNTIME_FINDINGS" ]; then
  jq -n -c \
    --arg id "runtime_network_targets" \
    --argjson tier 3 \
    --argjson points 3 \
    --arg detail "Runtime source contains hardcoded network targets: ${RUNTIME_FINDINGS}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
