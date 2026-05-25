#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

mkdir -p "$TMP_DIR/runtime-url/src" "$TMP_DIR/build-url"

cat > "$TMP_DIR/runtime-url/src/lib.rs" <<'RS'
pub const HELP_URL: &str = "https://example.invalid/help";
RS

runtime_report=$("$ROOT_DIR/analyzer/rules/hardcoded_urls.sh" "$TMP_DIR/runtime-url")
runtime_id=$(echo "$runtime_report" | jq -r '.id')
runtime_tier=$(echo "$runtime_report" | jq -r '.tier')
runtime_points=$(echo "$runtime_report" | jq -r '.points')

if [ "$runtime_id" != "runtime_network_targets" ] || [ "$runtime_tier" != "3" ] || [ "$runtime_points" != "3" ]; then
  echo "runtime URL should be an informational finding, got: $runtime_report" >&2
  exit 1
fi

cat > "$TMP_DIR/build-url/build.rs" <<'RS'
fn main() {
    println!("cargo:warning=https://attacker.invalid/bootstrap.sh");
}
RS

build_report=$("$ROOT_DIR/analyzer/rules/hardcoded_urls.sh" "$TMP_DIR/build-url")
build_id=$(echo "$build_report" | jq -r '.id')
build_tier=$(echo "$build_report" | jq -r '.tier')
build_points=$(echo "$build_report" | jq -r '.points')

if [ "$build_id" != "hardcoded_build_targets" ] || [ "$build_tier" != "1" ] || [ "$build_points" != "25" ]; then
  echo "build.rs URL should remain high signal, got: $build_report" >&2
  exit 1
fi

mkdir -p "$TMP_DIR/include-data/src" "$TMP_DIR/include-code/src"

cat > "$TMP_DIR/include-data/src/lib.rs" <<'RS'
pub const README: &str = include_str!("../README.md");
RS

include_data_report=$("$ROOT_DIR/analyzer/rules/include_macros.sh" "$TMP_DIR/include-data")
include_data_id=$(echo "$include_data_report" | jq -r '.id')
include_data_tier=$(echo "$include_data_report" | jq -r '.tier')
include_data_points=$(echo "$include_data_report" | jq -r '.points')

if [ "$include_data_id" != "include_data_macros" ] || [ "$include_data_tier" != "3" ] || [ "$include_data_points" != "3" ]; then
  echo "include_str/include_bytes should be informational, got: $include_data_report" >&2
  exit 1
fi

cat > "$TMP_DIR/include-code/src/lib.rs" <<'RS'
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
RS

include_code_report=$("$ROOT_DIR/analyzer/rules/include_macros.sh" "$TMP_DIR/include-code")
include_code_id=$(echo "$include_code_report" | jq -r '.id')
include_code_tier=$(echo "$include_code_report" | jq -r '.tier')
include_code_points=$(echo "$include_code_report" | jq -r '.points')

if [ "$include_code_id" != "include_code_macros" ] || [ "$include_code_tier" != "2" ] || [ "$include_code_points" != "10" ]; then
  echo "include! code substitution should remain review-worthy, got: $include_code_report" >&2
  exit 1
fi
