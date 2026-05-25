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

mkdir -p "$TMP_DIR/tokio-noise/src"

cat > "$TMP_DIR/tokio-noise/src/lib.rs" <<'RS'
//! assert_eq!(writer, b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09");
//! env::vars().filter(|&(ref k, _)| k != "SECRET");

use std::os::unix::ffi::OsStrExt;
use std::process::{Child as StdChild, Command as StdCommand};

pub fn socket_path() -> Vec<u8> {
    std::ffi::OsStr::new("/tmp/socket").as_bytes().to_vec()
}
RS

if "$ROOT_DIR/analyzer/rules/encoded_strings.sh" "$TMP_DIR/tokio-noise" | jq -e . >/dev/null; then
  echo "encoded strings in comments should not be flagged" >&2
  exit 1
fi

if "$ROOT_DIR/analyzer/rules/env_vars_bulk.sh" "$TMP_DIR/tokio-noise" | jq -e . >/dev/null; then
  echo "env::vars in comments should not be flagged" >&2
  exit 1
fi

if "$ROOT_DIR/analyzer/rules/dynamic_loading.sh" "$TMP_DIR/tokio-noise" | jq -e . >/dev/null; then
  echo "OsStrExt should not be treated as dynamic loading" >&2
  exit 1
fi

if "$ROOT_DIR/analyzer/rules/use_aliasing.sh" "$TMP_DIR/tokio-noise" | jq -e . >/dev/null; then
  echo "ordinary std aliases should not be treated as disguised imports" >&2
  exit 1
fi

mkdir -p "$TMP_DIR/unix-socket/src"

cat > "$TMP_DIR/unix-socket/src/lib.rs" <<'RS'
use std::os::unix::net::UnixListener;

pub fn bind(path: &str) -> std::io::Result<UnixListener> {
    UnixListener::bind(path)
}
RS

unix_report=$("$ROOT_DIR/analyzer/rules/unix_net_proc.sh" "$TMP_DIR/unix-socket")
unix_id=$(echo "$unix_report" | jq -r '.id')
unix_tier=$(echo "$unix_report" | jq -r '.tier')
unix_points=$(echo "$unix_report" | jq -r '.points')

if [ "$unix_id" != "unix_socket_usage" ] || [ "$unix_tier" != "3" ] || [ "$unix_points" != "3" ]; then
  echo "Unix socket usage should be informational, got: $unix_report" >&2
  exit 1
fi

mkdir -p "$TMP_DIR/diff-current/src" "$TMP_DIR/diff-prev/1.0.0/pkg-1.0.0/src"

cat > "$TMP_DIR/diff-current/Cargo.toml" <<'TOML'
[package]
name = "pkg"
version = "1.0.1"
edition = "2021"
TOML

cat > "$TMP_DIR/diff-current/src/lib.rs" <<'RS'
pub fn value() -> u8 { 1 }
RS

cat > "$TMP_DIR/diff-prev/1.0.0/pkg-1.0.0/Cargo.toml" <<'TOML'
[package]
name = "pkg"
version = "1.0.0"
edition = "2021"
TOML

cat > "$TMP_DIR/diff-prev/1.0.0/pkg-1.0.0/src/lib.rs" <<'RS'
pub fn value() -> u8 { 1 }
RS

if "$ROOT_DIR/analyzer/rules/massive_diff.sh" "$TMP_DIR/diff-current" "$TMP_DIR/diff-prev" | jq -e . >/dev/null; then
  echo "nested previous tarball roots should not cause massive_diff" >&2
  exit 1
fi
