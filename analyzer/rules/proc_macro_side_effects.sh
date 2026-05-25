#!/usr/bin/env bash
set -euo pipefail

# Tier 2 (10 points): Detect side effects in proc-macro crates
# If Cargo.toml has proc-macro = true, search src/ for network, process, or fs calls

SOURCE_DIR="${1:?Usage: proc_macro_side_effects.sh <source_dir>}"

CARGO_TOML="${SOURCE_DIR}/Cargo.toml"

[ -f "$CARGO_TOML" ] || exit 0

# Check if this is a proc-macro crate
IS_PROC_MACRO=$(rg -c 'proc-macro\s*=\s*true' "$CARGO_TOML" 2>/dev/null || echo "0")

[ "$IS_PROC_MACRO" -eq 0 ] && exit 0

SRC_DIR="${SOURCE_DIR}/src"

[ -d "$SRC_DIR" ] || exit 0

# Search for suspicious side-effect patterns in src/
SIDE_EFFECT_PATTERNS='std::net::|tokio::net::|reqwest|ureq|hyper|TcpStream|UdpSocket|Command::new|std::process::Command|std::fs::write|std::fs::create_dir|std::fs::remove|File::create'

MATCHES=$(rg -n "$SIDE_EFFECT_PATTERNS" "$SRC_DIR" --type rust 2>/dev/null || true)

if [ -n "$MATCHES" ]; then
  COUNT=$(echo "$MATCHES" | wc -l | tr -d ' ')
  FIRST=$(echo "$MATCHES" | head -1 | cut -c1-150)
  jq -n -c \
    --arg id "proc_macro_side_effects" \
    --argjson tier 2 \
    --argjson points 10 \
    --arg detail "proc-macro crate has side-effect code in src/ (${COUNT} matches): network, process, or filesystem calls. First: ${FIRST}" \
    '{id: $id, tier: $tier, points: $points, detail: $detail}'
fi
