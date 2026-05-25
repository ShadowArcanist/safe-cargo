#!/usr/bin/env bash
set -euo pipefail

# Tier 1 (25 points): Detect inline assembly and FFI declarations
# Flags: asm!, global_asm!, core::arch::asm!, std::arch::asm!, #[link(, extern "C" { blocks

SOURCE_DIR="${1:?Usage: asm_ffi.sh <source_dir>}"

FINDINGS=""

# Detect inline assembly: asm!, global_asm!, core::arch::asm!, std::arch::asm!
ASM_MATCHES=$(rg -n 'asm!\s*\(|global_asm!\s*\(|core::arch::asm!\s*\(|std::arch::asm!\s*\(' "$SOURCE_DIR" --type rust 2>/dev/null || true)

if [ -n "$ASM_MATCHES" ]; then
  COUNT=$(echo "$ASM_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$ASM_MATCHES" | head -1 | cut -c1-120)
  FINDINGS="${FINDINGS}inline assembly (${COUNT} matches, first: ${FIRST}); "
fi

# Detect #[link( attribute — linking external C libraries
LINK_MATCHES=$(rg -n '#\[link\(' "$SOURCE_DIR" --type rust 2>/dev/null || true)

if [ -n "$LINK_MATCHES" ]; then
  COUNT=$(echo "$LINK_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$LINK_MATCHES" | head -1 | cut -c1-120)
  FINDINGS="${FINDINGS}external library linking (${COUNT} matches, first: ${FIRST}); "
fi

# Detect extern "C" { blocks (FFI imports) — exclude extern "C" fn (safe calling convention)
EXTERN_MATCHES=$(rg -n 'extern\s+"C"\s*\{' "$SOURCE_DIR" --type rust 2>/dev/null || true)

if [ -n "$EXTERN_MATCHES" ]; then
  COUNT=$(echo "$EXTERN_MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$EXTERN_MATCHES" | head -1 | cut -c1-120)
  FINDINGS="${FINDINGS}FFI extern \"C\" blocks (${COUNT} matches, first: ${FIRST}); "
fi

if [ -n "$FINDINGS" ]; then
  jq -n -c \
    --arg id "asm_ffi" \
    --argjson tier 1 \
    --argjson points 25 \
    --arg detail "Inline assembly or FFI detected: ${FINDINGS}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
