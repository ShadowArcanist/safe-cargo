#!/usr/bin/env bash
set -euo pipefail

# Tier 1 (25 points): Detect encoded/obfuscated strings in source files
# Flags: base64 strings, hex sequences, suspiciously long non-readable strings

SOURCE_DIR="${1:?Usage: encoded_strings.sh <source_dir>}"

# Search ALL source files (not just build.rs)

# Base64-like strings: long alphanumeric with optional padding (40+ chars)
B64_MATCHES=$(rg -n '"[A-Za-z0-9+/]{40,}={0,2}"' "$SOURCE_DIR" --type rust 2>/dev/null || true)

# Hex-encoded strings: \x sequences longer than 20 chars
HEX_MATCHES=$(rg -n '(\\x[0-9a-fA-F]{2}){10,}' "$SOURCE_DIR" --type rust 2>/dev/null || true)

# Suspiciously long string literals (>200 chars of non-readable content)
# Matches strings with mostly non-alphabetic printable chars
LONG_MATCHES=$(rg --no-filename -n '"[^"]{200,}"' "$SOURCE_DIR" --type rust 2>/dev/null | \
  rg -v '(https?://|/\*|\*/|//|\btest\b|\bdoc\b|\blicense\b|\bcopyright\b|README)' 2>/dev/null || true)

# Also check raw byte strings
RAW_B64=$(rg -n 'b"[A-Za-z0-9+/]{40,}={0,2}"' "$SOURCE_DIR" --type rust 2>/dev/null || true)
# Check for large byte arrays (potential obfuscated data)
BYTE_ARRAYS=$(rg -n '\[0x[0-9a-fA-F]{2}(,\s*0x[0-9a-fA-F]{2}){19,}\]' "$SOURCE_DIR" --type rust 2>/dev/null || true)

FINDINGS=""

if [ -n "$B64_MATCHES" ]; then
  COUNT=$(echo "$B64_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$B64_MATCHES" | head -1 | cut -c1-120)
  FINDINGS="${FINDINGS}base64-like strings (${COUNT} matches, first: ${FIRST}); "
fi

if [ -n "$HEX_MATCHES" ]; then
  COUNT=$(echo "$HEX_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$HEX_MATCHES" | head -1 | cut -c1-120)
  FINDINGS="${FINDINGS}hex-encoded sequences (${COUNT} matches, first: ${FIRST}); "
fi

if [ -n "$LONG_MATCHES" ]; then
  COUNT=$(echo "$LONG_MATCHES" | wc -l | tr -d ' ')
  FINDINGS="${FINDINGS}suspiciously long string literals (${COUNT} matches); "
fi

if [ -n "$RAW_B64" ]; then
  COUNT=$(echo "$RAW_B64" | wc -l | tr -d ' ')
  FIRST=$(echo "$RAW_B64" | head -1 | cut -c1-120)
  FINDINGS="${FINDINGS}raw byte base64-like strings (${COUNT} matches, first: ${FIRST}); "
fi

if [ -n "$BYTE_ARRAYS" ]; then
  COUNT=$(echo "$BYTE_ARRAYS" | wc -l | tr -d ' ')
  FIRST=$(echo "$BYTE_ARRAYS" | head -1 | cut -c1-120)
  FINDINGS="${FINDINGS}large byte arrays (${COUNT} matches, first: ${FIRST}); "
fi

if [ -n "$FINDINGS" ]; then
  jq -n -c \
    --arg id "encoded_strings" \
    --argjson tier 1 \
    --argjson points 25 \
    --arg detail "Potential obfuscated content found: ${FINDINGS}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
