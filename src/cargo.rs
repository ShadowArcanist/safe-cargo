use std::process::Command;

use crate::error::{Error, Result};

/// A single package entry parsed from `Cargo.lock`.
pub struct LockPackage {
    pub name: String,
    pub version: String,
}

/// Parse `Cargo.lock` from the current directory and return all crates.io
/// packages (skipping the root/workspace packages and path/git dependencies).
pub fn parse_cargo_lock() -> Result<Vec<LockPackage>> {
    let lock_path = std::env::current_dir()?.join("Cargo.lock");
    let contents = std::fs::read_to_string(&lock_path)
        .map_err(|e| Error::CargoCommandFailed(format!("failed to read Cargo.lock: {e}")))?;

    let parsed: toml::Value = toml::from_str(&contents)?;

    let packages_array = match parsed.get("package").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Ok(Vec::new()),
    };

    let mut result = Vec::new();

    for entry in packages_array {
        let table = match entry.as_table() {
            Some(t) => t,
            None => continue,
        };

        let name = match table.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => continue,
        };

        let version = match table.get("version").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => continue,
        };

        // Only include crates.io registry packages.
        // Packages without a `source` field are local/root packages — skip them.
        let source = match table.get("source").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };

        if !source.starts_with("registry+") {
            continue;
        }

        if crate::validate::crate_name(name).is_err() {
            continue; // skip invalid entries
        }

        result.push(LockPackage {
            name: name.to_string(),
            version: version.to_string(),
        });
    }

    Ok(result)
}

/// Read the root crate name and version from `Cargo.toml` in the current
/// directory.
pub fn read_root_crate() -> Result<(String, String)> {
    let cargo_toml_path = std::env::current_dir()?.join("Cargo.toml");
    let contents = std::fs::read_to_string(&cargo_toml_path)
        .map_err(|e| Error::CargoCommandFailed(format!("failed to read Cargo.toml: {e}")))?;

    let parsed: toml::Value = toml::from_str(&contents)?;

    let package = parsed
        .get("package")
        .and_then(|v| v.as_table())
        .ok_or_else(|| Error::ConfigParse {
            path: cargo_toml_path.clone(),
            reason: "missing [package] section in Cargo.toml".to_string(),
        })?;

    let name = package
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::ConfigParse {
            path: cargo_toml_path.clone(),
            reason: "missing package.name in Cargo.toml".to_string(),
        })?
        .to_string();

    let version = package
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::ConfigParse {
            path: cargo_toml_path.clone(),
            reason: "missing package.version in Cargo.toml".to_string(),
        })?
        .to_string();

    Ok((name, version))
}

/// Run `cargo` with the given arguments, inheriting stdin/stdout/stderr.
///
/// Returns `Ok(())` on a zero exit code, or an error describing the failure.
pub fn run(args: &[&str]) -> Result<()> {
    let status = Command::new("cargo")
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::CargoNotInstalled
            } else {
                Error::CargoCommandFailed(format!("failed to launch cargo: {e}"))
            }
        })?;

    if status.success() {
        Ok(())
    } else {
        let code = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        Err(Error::CargoCommandFailed(format!(
            "cargo {} exited with code {code}",
            args.join(" ")
        )))
    }
}

/// Run `cargo` with the given arguments, capturing stdout.
///
/// Stderr is inherited so the user still sees warnings/errors.
/// Returns the captured stdout as a `String`.
#[allow(dead_code)]
pub fn run_capture(args: &[&str]) -> Result<String> {
    let output = Command::new("cargo")
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::CargoNotInstalled
            } else {
                Error::CargoCommandFailed(format!("failed to launch cargo: {e}"))
            }
        })?;

    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| {
            Error::CargoCommandFailed(format!("cargo output was not valid UTF-8: {e}"))
        })
    } else {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        Err(Error::CargoCommandFailed(format!(
            "cargo {} exited with code {code}",
            args.join(" ")
        )))
    }
}

#[allow(dead_code)]
pub fn is_installed() -> bool {
    Command::new("cargo")
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
