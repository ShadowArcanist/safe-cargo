# safe-cargo

A cargo wrapper that protects Rust projects from supply-chain attacks. Every dependency is analyzed for malicious patterns before it touches your project.

> [!WARNING]
> This is an experiment. If you find this idea useful, feel free to use the code to build your own.

## How it works

1. You run `safe-cargo add tokio` instead of `cargo add tokio`
2. safe-cargo fetches security analysis reports from a central registry (this GitHub repo)
3. Each report is cryptographically signed and scored against 21 detection rules
4. If the crate and all its transitive dependencies pass your policy, the crate is added
5. If anything fails, the addition is blocked

Analysis runs on GitHub Actions. Reports are signed with [minisign](https://jedisct1.github.io/minisign/) and stored in Git.

## Detection rules

21 rules across 3 severity tiers scan for:

| Tier | Points | Examples |
|------|--------|---------|
| High | 25 | Network in build.rs, env var exfiltration, hardcoded URLs, encoded strings, process spawning |
| Medium | 10 | New maintainers, massive diffs, new unsafe blocks, proc macro side effects, build.rs changes |
| Low | 3 | Release age, build.rs presence, new dependency additions |

Crates are scored and given a verdict: **PASS** (0-15), **WARN** (16-40), or **FAIL** (41+).

## Install

```sh
git clone https://github.com/ShadowArcanist/safe-cargo.git
cd safe-cargo
cargo build --release
```

Binary will be at `target/release/safe-cargo`. Add it to your `PATH` or copy it somewhere in your `PATH`:

```sh
cp target/release/safe-cargo /usr/local/bin/
```

## Setup

```sh
safe-cargo setup
```

This walks you through configuring:
- GitHub registry repo and branch
- GitHub token (stored in your OS keychain)
- minisign public key for signature verification

Creates a `safe-cargo.toml` in your project.

## Usage

```sh
# Add a crate (checks all transitive deps first)
safe-cargo add serde
safe-cargo add tokio@1.38.0

# Check all dependencies in Cargo.lock
safe-cargo check

# Show analysis status for all deps
safe-cargo status

# Manually allow a crate (with required reason)
safe-cargo allow openssl-sys@0.9.102 --reason "Reviewed internally"

# Trigger analysis for unanalyzed deps
safe-cargo audit

# Update deps and verify new versions
safe-cargo update

# Any other cargo command passes through
safe-cargo build
safe-cargo test
```

## Configuration

`safe-cargo.toml`:

```toml
[registry]
repo = "your-org/safe-cargo-reports"
branch = "master"
signing_key = "RW..."

[policy]
max_score = 40
min_release_age = "3d"
require_report = true

[allow]
"my-internal-crate@1.0.0" = { version = "1.0.0", reason = "Internal crate, reviewed by team" }
```

| Setting | Description |
|---------|-------------|
| `max_score` | Maximum allowed risk score (default: 40) |
| `min_release_age` | Minimum time since crate was published (e.g. `3d`, `1w`) |
| `require_report` | Block crates without analysis reports |

## Architecture

```
safe-cargo (CLI)          GitHub Actions (Analyzer)
  |                         |
  |-- validates input       |-- downloads crate source
  |-- resolves versions     |-- verifies checksums
  |-- fetches reports       |-- runs 21 detection rules
  |-- verifies signatures   |-- scores + verdicts
  |-- enforces policy       |-- signs reports with minisign
  |-- adds/blocks crate     |-- commits to registry repo
```

Reports live in a GitHub repo you control. The CLI fetches and verifies them locally. Analysis and signing happen in CI. Your project never trusts unverified data.

## License

MIT
