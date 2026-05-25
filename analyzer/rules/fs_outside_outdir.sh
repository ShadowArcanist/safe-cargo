#!/usr/bin/env bash
set -euo pipefail

# Tier 2 (10 points): Detect filesystem writes in build.rs to paths NOT using OUT_DIR
# Flags: std::fs::write, std::fs::create_dir, File::create without OUT_DIR

SOURCE_DIR="${1:?Usage: fs_outside_outdir.sh <source_dir>}"

BUILD_RS="${SOURCE_DIR}/build.rs"

[ -f "$BUILD_RS" ] || exit 0

# Find filesystem write operations
FS_PATTERNS='std::fs::write|std::fs::create_dir|std::fs::remove|File::create|fs::write|fs::create_dir|fs::remove'

MATCHES=$(rg -n "$FS_PATTERNS" "$BUILD_RS" 2>/dev/null || true)

[ -z "$MATCHES" ] && exit 0

# Filter out lines that reference OUT_DIR (which is the safe/expected target)
# Strip comments first, then check if OUT_DIR is in the actual code
SUSPICIOUS=""
while IFS= read -r line; do
  # Extract code portion (before //)
  CODE_PART=$(echo "$line" | sed 's|//.*$||')
  if echo "$CODE_PART" | rg -q 'OUT_DIR|out_dir' 2>/dev/null; then
    continue  # OUT_DIR is in the code, not just a comment
  fi
  if [ -n "$SUSPICIOUS" ]; then
    SUSPICIOUS="${SUSPICIOUS}
${line}"
  else
    SUSPICIOUS="${line}"
  fi
done <<< "$MATCHES"

if [ -n "$SUSPICIOUS" ]; then
  COUNT=$(echo "$SUSPICIOUS" | wc -l | tr -d ' ')
  FIRST=$(echo "$SUSPICIOUS" | head -1 | cut -c1-150)
  jq -n -c \
    --arg id "fs_outside_outdir" \
    --argjson tier 2 \
    --argjson points 10 \
    --arg detail "build.rs writes to filesystem outside OUT_DIR (${COUNT} occurrences). First: ${FIRST}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
