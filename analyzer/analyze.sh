#!/usr/bin/env bash
set -euo pipefail

# Main analysis orchestrator for a single crate
# Usage: ./analyzer/analyze.sh <source_dir> <crate_name> <version> <prev_versions_dir>

SOURCE_DIR="${1:?Usage: analyze.sh <source_dir> <crate_name> <version> <prev_versions_dir>}"
CRATE_NAME="${2:?Missing crate name}"
VERSION="${3:?Missing version}"
PREV_VERSIONS_DIR="${4:?Missing prev_versions_dir}"

# Validate inputs
if [[ ! "$CRATE_NAME" =~ ^[a-zA-Z][a-zA-Z0-9_-]{0,63}$ ]]; then
  echo "ERROR: Invalid crate name: $CRATE_NAME" >&2
  exit 1
fi
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?(\+[a-zA-Z0-9.]+)?$ ]]; then
  echo "ERROR: Invalid version: $VERSION" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULES_DIR="${SCRIPT_DIR}/rules"

SAFE_TMPDIR=$(mktemp -d)
trap 'rm -rf "$SAFE_TMPDIR"' EXIT

# Collect all findings from rule scripts
FINDINGS="[]"
RULES_EXECUTED="[]"
RULES_FAILED="[]"

run_rule() {
  local rule_script="$1"
  local rule_name
  rule_name=$(basename "$rule_script" .sh)
  shift
  local output
  local exit_code
  output=$("$rule_script" "$@" 2>"$SAFE_TMPDIR/rule_stderr_$rule_name") && exit_code=0 || exit_code=$?

  RULES_EXECUTED=$(echo "$RULES_EXECUTED" | jq --arg r "$rule_name" '. + [$r]')

  if [ "$exit_code" -ne 0 ] && [ -z "$output" ]; then
    # Rule failed without producing output — record failure (fail-safe: 25 points)
    RULES_FAILED=$(echo "$RULES_FAILED" | jq --arg r "$rule_name" '. + [$r]')
    STDERR_MSG=$(cat "$SAFE_TMPDIR/rule_stderr_$rule_name" 2>/dev/null | head -1 || echo "unknown error")
    FINDINGS=$(echo "$FINDINGS" | jq --arg id "${rule_name}_error" \
      --arg detail "Rule '${rule_name}' failed to execute (fail-safe scoring applied): ${STDERR_MSG}" \
      '. + [{"id": $id, "tier": 1, "points": 25, "detail": $detail}]')
  elif [ -n "$output" ]; then
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      if echo "$line" | jq empty 2>/dev/null; then
        FINDINGS=$(echo "$FINDINGS" | jq --argjson f "$line" '. + [$f]')
      else
        # Non-JSON output from rule — treat as potential finding
        FINDINGS=$(echo "$FINDINGS" | jq --arg id "${rule_name}_malformed" \
          --arg detail "Rule produced non-JSON output: $(echo "$line" | cut -c1-100)" \
          '. + [{"id": $id, "tier": 2, "points": 10, "detail": $detail}]')
      fi
    done <<< "$output"
  fi
  rm -f "$SAFE_TMPDIR/rule_stderr_$rule_name"
}

# ── Tier 1 rules (25 points each) ──
run_rule "${RULES_DIR}/network_build_rs.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/process_spawn.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/encoded_strings.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/env_exfil.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/hardcoded_urls.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/include_macros.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/env_vars_bulk.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/asm_ffi.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/dynamic_loading.sh" "$SOURCE_DIR"

# ── Tier 2 rules (10 points each) ──
run_rule "${RULES_DIR}/build_rs_diff.sh" "$SOURCE_DIR" "$PREV_VERSIONS_DIR"
run_rule "${RULES_DIR}/fs_outside_outdir.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/proc_macro_side_effects.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/massive_diff.sh" "$SOURCE_DIR" "$PREV_VERSIONS_DIR"
run_rule "${RULES_DIR}/new_maintainer.sh" "$CRATE_NAME" "$VERSION" "$PREV_VERSIONS_DIR"
run_rule "${RULES_DIR}/network_runtime.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/unix_net_proc.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/use_aliasing.sh" "$SOURCE_DIR"

# ── Tier 3 rules (3 points each) ──
run_rule "${RULES_DIR}/unsafe_blocks.sh" "$SOURCE_DIR" "$PREV_VERSIONS_DIR"
run_rule "${RULES_DIR}/dep_additions.sh" "$SOURCE_DIR" "$PREV_VERSIONS_DIR"
run_rule "${RULES_DIR}/build_rs_present.sh" "$SOURCE_DIR"
run_rule "${RULES_DIR}/release_age.sh" "$CRATE_NAME" "$VERSION"

# Calculate total score
TOTAL_SCORE=$(echo "$FINDINGS" | jq '[.[].points] | add // 0')

# Determine verdict
if [ "$TOTAL_SCORE" -le 15 ]; then
  VERDICT="PASS"
elif [ "$TOTAL_SCORE" -le 40 ]; then
  VERDICT="WARN"
else
  VERDICT="FAIL"
fi

# Fetch metadata from crates.io
CRATE_META=""
OWNERS=""
REPO=""
DOWNLOADS=""
PUBLISHED=""

CRATE_META=$(curl -sf "https://crates.io/api/v1/crates/${CRATE_NAME}/${VERSION}" \
  -H "User-Agent: safe-cargo-analyzer" 2>/dev/null || echo "{}")

PUBLISHED=$(echo "$CRATE_META" | jq -r '.version.created_at // empty' 2>/dev/null || echo "")
DOWNLOADS=$(echo "$CRATE_META" | jq -r '.version.downloads // 0' 2>/dev/null || echo "0")
[ -z "$DOWNLOADS" ] && DOWNLOADS="0"

CRATE_INFO=$(curl -sf "https://crates.io/api/v1/crates/${CRATE_NAME}" \
  -H "User-Agent: safe-cargo-analyzer" 2>/dev/null || echo "{}")
REPO=$(echo "$CRATE_INFO" | jq -r '.crate.repository // empty' 2>/dev/null || echo "")

OWNERS_JSON=$(curl -sf "https://crates.io/api/v1/crates/${CRATE_NAME}/owners" \
  -H "User-Agent: safe-cargo-analyzer" 2>/dev/null || echo '{"users":[]}')
OWNERS=$(echo "$OWNERS_JSON" | jq '[.users[]?.login] // []' 2>/dev/null || echo "[]")

# Calculate age in days since publish
AGE_DAYS=""
if [ -n "$PUBLISHED" ]; then
  PUBLISH_EPOCH=$(date -d "$PUBLISHED" +%s 2>/dev/null || date -j -f "%Y-%m-%dT%H:%M:%S" "${PUBLISHED%%.*}" +%s 2>/dev/null || echo "")
  if [ -n "$PUBLISH_EPOCH" ]; then
    NOW_EPOCH=$(date +%s)
    AGE_DAYS=$(( (NOW_EPOCH - PUBLISH_EPOCH) / 86400 ))
  fi
fi

# Compute delta stats against previous versions
COMPARED_VERSIONS="[]"
LINES_ADDED=0
LINES_REMOVED=0
NEW_FILES="[]"
BUILD_RS_CHANGED="false"
DEPS_ADDED="[]"
DEPS_REMOVED="[]"

if [ -d "$PREV_VERSIONS_DIR" ]; then
  # Collect all previous version directory names
  COMPARED_VERSIONS=$(ls -1 "$PREV_VERSIONS_DIR" 2>/dev/null | sort -V | jq -R -s 'split("\n") | map(select(. != ""))' || echo "[]")

  # Find the most recent previous version directory for diffing
  LATEST_PREV=$(ls -1 "$PREV_VERSIONS_DIR" 2>/dev/null | sort -V | tail -1 || true)
  if [ -n "$LATEST_PREV" ] && [ -d "${PREV_VERSIONS_DIR}/${LATEST_PREV}" ]; then
    # Count lines added and removed
    DIFF_FULL=$(diff -r "$SOURCE_DIR" "${PREV_VERSIONS_DIR}/${LATEST_PREV}" 2>/dev/null || true)
    if [ -n "$DIFF_FULL" ]; then
      LINES_ADDED=$(echo "$DIFF_FULL" | grep -c '^< ' || true)
      LINES_REMOVED=$(echo "$DIFF_FULL" | grep -c '^> ' || true)
      [ -z "$LINES_ADDED" ] && LINES_ADDED=0
      [ -z "$LINES_REMOVED" ] && LINES_REMOVED=0
    fi

    # Detect new files (files only in SOURCE_DIR)
    # Use awk instead of sed to avoid SOURCE_DIR metacharacter issues
    NEW_FILES=$(diff -rq "$SOURCE_DIR" "${PREV_VERSIONS_DIR}/${LATEST_PREV}" 2>/dev/null \
      | grep "^Only in ${SOURCE_DIR}" \
      | awk -v prefix="$SOURCE_DIR" '{
          sub("^Only in " prefix "[/]*: *", "");
          print
        }' \
      | jq -R -s 'split("\n") | map(select(. != ""))' || echo "[]")

    # Check if build.rs changed
    if [ -f "${SOURCE_DIR}/build.rs" ] && [ -f "${PREV_VERSIONS_DIR}/${LATEST_PREV}/build.rs" ]; then
      if ! diff -q "${SOURCE_DIR}/build.rs" "${PREV_VERSIONS_DIR}/${LATEST_PREV}/build.rs" > /dev/null 2>&1; then
        BUILD_RS_CHANGED="true"
      fi
    elif [ -f "${SOURCE_DIR}/build.rs" ] || [ -f "${PREV_VERSIONS_DIR}/${LATEST_PREV}/build.rs" ]; then
      BUILD_RS_CHANGED="true"
    fi

    # Compare dependencies using TOML-aware parsing
    DEP_DIFF=$(python3 -c "
import tomllib, json, sys

def get_deps(path):
    try:
        with open(path, 'rb') as f:
            data = tomllib.load(f)
        deps = set(data.get('dependencies', {}).keys())
        deps.update(data.get('build-dependencies', {}).keys())
        return deps
    except Exception:
        return set()

current = get_deps(sys.argv[1])
previous = get_deps(sys.argv[2])
print(json.dumps({
    'added': sorted(current - previous),
    'removed': sorted(previous - current)
}))
" "${SOURCE_DIR}/Cargo.toml" "${PREV_VERSIONS_DIR}/${LATEST_PREV}/Cargo.toml" 2>/dev/null || echo '{"added":[],"removed":[]}')
    DEPS_ADDED=$(echo "$DEP_DIFF" | jq '.added' 2>/dev/null || echo "[]")
    DEPS_REMOVED=$(echo "$DEP_DIFF" | jq '.removed' 2>/dev/null || echo "[]")
  fi
fi

echo "DEBUG: Building delta JSON..." >&2
echo "DEBUG: LINES_ADDED=${LINES_ADDED:-0} LINES_REMOVED=${LINES_REMOVED:-0}" >&2
DELTA=$(jq -n \
  --argjson compared_versions "${COMPARED_VERSIONS:-[]}" \
  --argjson lines_added "${LINES_ADDED:-0}" \
  --argjson lines_removed "${LINES_REMOVED:-0}" \
  --argjson new_files "${NEW_FILES:-[]}" \
  --argjson build_rs_changed "${BUILD_RS_CHANGED:-false}" \
  --argjson deps_added "${DEPS_ADDED:-[]}" \
  --argjson deps_removed "${DEPS_REMOVED:-[]}" \
  '{
    compared_versions: $compared_versions,
    lines_added: $lines_added,
    lines_removed: $lines_removed,
    new_files: $new_files,
    build_rs_changed: $build_rs_changed,
    deps_added: $deps_added,
    deps_removed: $deps_removed
  }')

echo "DEBUG: Building final report JSON..." >&2
echo "DEBUG: TOTAL_SCORE=${TOTAL_SCORE:-?} DOWNLOADS=${DOWNLOADS:-?} AGE_DAYS=${AGE_DAYS:-?}" >&2
jq -n \
  --arg crate_name "$CRATE_NAME" \
  --arg version "$VERSION" \
  --arg published_at "$PUBLISHED" \
  --argjson release_age_days "${AGE_DAYS:-0}" \
  --argjson score "${TOTAL_SCORE:-0}" \
  --arg verdict "${VERDICT:-FAIL}" \
  --argjson triggered_rules "${FINDINGS:-[]}" \
  --argjson downloads "${DOWNLOADS:-0}" \
  --arg repo "${REPO:-}" \
  --argjson owners "${OWNERS:-[]}" \
  --argjson delta "${DELTA:-{}}" \
  --argjson rules_executed "${RULES_EXECUTED:-[]}" \
  --argjson rules_failed "${RULES_FAILED:-[]}" \
  '{
    crate_name: $crate_name,
    version: $version,
    published_at: (if $published_at != "" then $published_at else null end),
    analyzed_at: (now | todate),
    release_age_days: $release_age_days,
    score: $score,
    verdict: $verdict,
    triggered_rules: $triggered_rules,
    rules_executed: $rules_executed,
    rules_failed: $rules_failed,
    delta: $delta,
    metadata: {
      owners: $owners,
      repository: (if $repo != "" then $repo else null end),
      downloads: $downloads
    }
  }'
