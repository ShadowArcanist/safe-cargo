#!/usr/bin/env bash
set -euo pipefail

# Tier 1 (25 points): Detect dynamic library loading at runtime
# Flags: libloading crate, dlopen, LoadLibrary, std::os::unix::ffi with dynamic loading

SOURCE_DIR="${1:?Usage: dynamic_loading.sh <source_dir>}"

FINDINGS=""

filter_code_lines() {
  rg -v ':[[:space:]]*(//|///|//!|/\*|\*)' 2>/dev/null || true
}

first_line() {
  awk 'NR == 1 { print; exit }' <<< "$1" | cut -c1-120
}

# Detect libloading crate usage
LIBLOADING_MATCHES=$(rg -n 'libloading::Library|Library::new|libloading::Symbol' "$SOURCE_DIR" --type rust 2>/dev/null | filter_code_lines || true)

if [ -n "$LIBLOADING_MATCHES" ]; then
  COUNT=$(echo "$LIBLOADING_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(first_line "$LIBLOADING_MATCHES")
  FINDINGS="${FINDINGS}libloading crate usage (${COUNT} matches, first: ${FIRST}); "
fi

# Detect dlopen calls
DLOPEN_MATCHES=$(rg -n 'dlopen|dlsym|dlclose' "$SOURCE_DIR" --type rust 2>/dev/null | filter_code_lines || true)

if [ -n "$DLOPEN_MATCHES" ]; then
  COUNT=$(echo "$DLOPEN_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(first_line "$DLOPEN_MATCHES")
  FINDINGS="${FINDINGS}dlopen/dlsym calls (${COUNT} matches, first: ${FIRST}); "
fi

# Detect Windows LoadLibrary
LOADLIB_MATCHES=$(rg -n 'LoadLibrary|GetProcAddress' "$SOURCE_DIR" --type rust 2>/dev/null | filter_code_lines || true)

if [ -n "$LOADLIB_MATCHES" ]; then
  COUNT=$(echo "$LOADLIB_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(first_line "$LOADLIB_MATCHES")
  FINDINGS="${FINDINGS}Windows dynamic loading (${COUNT} matches, first: ${FIRST}); "
fi

if [ -n "$FINDINGS" ]; then
  jq -n -c \
    --arg id "dynamic_loading" \
    --argjson tier 1 \
    --argjson points 25 \
    --arg detail "Dynamic library loading detected: ${FINDINGS}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
