#!/usr/bin/env bash
set -euo pipefail

# Tier 1 (25 points): Detect process spawning in build.rs
# Flags: Command::new, std::process::Command
# Excludes known-safe patterns: cc::Build, bindgen, pkg_config, cmake

SOURCE_DIR="${1:?Usage: process_spawn.sh <source_dir>}"

BUILD_RS="${SOURCE_DIR}/build.rs"

[ -f "$BUILD_RS" ] || exit 0

# Search for process spawn patterns
MATCHES=$(rg -n 'Command::new|std::process::Command' "$BUILD_RS" 2>/dev/null || true)

[ -z "$MATCHES" ] && exit 0

# Filter out known-safe patterns
SAFE_TOOLS="cc|gcc|g\+\+|c\+\+|clang|clang\+\+|ar|ranlib|bindgen|pkg-config|cmake|make|rustc|rustfmt"
SUSPICIOUS=""
while IFS= read -r line; do
  [ -z "$line" ] && continue
  CODE_PART=$(echo "$line" | sed 's|//.*$||')
  [ -z "$CODE_PART" ] && continue
  CMD_ARG=$(echo "$CODE_PART" | rg -o 'Command::new\(\s*"([^"]*)"' -r '$1' 2>/dev/null || true)
  if [ -n "$CMD_ARG" ]; then
    if echo "$CMD_ARG" | rg -qx "$SAFE_TOOLS" 2>/dev/null; then
      continue
    fi
  fi
  if echo "$CODE_PART" | rg -q 'cc::Build|bindgen::Builder|pkg_config::|cmake::Config' 2>/dev/null; then
    continue
  fi
  SUSPICIOUS="${SUSPICIOUS}${line}
"
done <<< "$MATCHES"
SUSPICIOUS=$(echo "$SUSPICIOUS" | sed '/^$/d')

if [ -n "$SUSPICIOUS" ]; then
  MATCH_COUNT=$(echo "$SUSPICIOUS" | wc -l | tr -d ' ')
  FIRST_MATCH=$(echo "$SUSPICIOUS" | head -1 | tr -d '\n')
  jq -n -c \
    --arg id "process_spawn" \
    --argjson tier 1 \
    --argjson points 25 \
    --arg detail "build.rs spawns processes (${MATCH_COUNT} occurrences, excluding safe patterns). First: ${FIRST_MATCH}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
