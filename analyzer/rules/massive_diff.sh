#!/usr/bin/env bash
set -euo pipefail

# Tier 2 (10 points): Detect massive diffs for patch version bumps
# >500 lines changed for a patch version bump = flag

SOURCE_DIR="${1:?Usage: massive_diff.sh <source_dir> <prev_versions_dir>}"
PREV_VERSIONS_DIR="${2:?Missing prev_versions_dir}"

[ -d "$PREV_VERSIONS_DIR" ] || exit 0

# Find the most recent previous version
LATEST_PREV=$(ls -1 "$PREV_VERSIONS_DIR" 2>/dev/null | sort -V | tail -1 || true)
[ -z "$LATEST_PREV" ] && exit 0
[ -d "${PREV_VERSIONS_DIR}/${LATEST_PREV}" ] || exit 0

DIFF_BRIEF=$(diff -r --brief "$SOURCE_DIR" "${PREV_VERSIONS_DIR}/${LATEST_PREV}" 2>/dev/null || true)

# Count changed lines between versions
DIFF_LINES=$(printf '%s\n' "$DIFF_BRIEF" | wc -l | tr -d ' ')
[ -z "$DIFF_BRIEF" ] && DIFF_LINES=0

# Get a more detailed line count
CHANGED_LINES=0
while IFS= read -r line; do
  # For each differing file, count the actual line changes
  FILE_A=$(echo "$line" | sed -n 's/^Files \(.*\) and .* differ$/\1/p' 2>/dev/null || true)
  if [ -n "$FILE_A" ]; then
    FILE_B=$(echo "$line" | sed -n 's/^Files .* and \(.*\) differ$/\1/p' 2>/dev/null || true)
    if [ -n "$FILE_B" ]; then
      FILE_DIFF=$(diff "$FILE_A" "$FILE_B" 2>/dev/null | grep -c '^[<>]' || true)
      [ -z "$FILE_DIFF" ] && FILE_DIFF=0
      CHANGED_LINES=$((CHANGED_LINES + FILE_DIFF))
    fi
  fi
done <<< "$DIFF_BRIEF"

# Also count lines from files only in one version
ONLY_IN_CURRENT=$(printf '%s\n' "$DIFF_BRIEF" | awk -v dir="$SOURCE_DIR" 'index($0, "Only in " dir) == 1' | wc -l | tr -d ' ')
ONLY_IN_PREV=$(printf '%s\n' "$DIFF_BRIEF" | awk -v dir="${PREV_VERSIONS_DIR}/${LATEST_PREV}" 'index($0, "Only in " dir) == 1' | wc -l | tr -d ' ')

TOTAL_CHANGES=$((CHANGED_LINES + ONLY_IN_CURRENT * 50 + ONLY_IN_PREV * 50))

if [ "$TOTAL_CHANGES" -gt 500 ]; then
  jq -n -c \
    --arg id "massive_diff" \
    --argjson tier 2 \
    --argjson points 10 \
    --argjson changed "$TOTAL_CHANGES" \
    --arg prev "$LATEST_PREV" \
    --arg detail "Massive diff detected: ~${TOTAL_CHANGES} lines changed vs previous version ${LATEST_PREV}. ${DIFF_LINES} files differ, ${ONLY_IN_CURRENT} files added, ${ONLY_IN_PREV} files removed." \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
