use colored::Colorize;

use crate::error::{Error, Result};

/// Add a crate to the allowlist in `safe-cargo.toml`.
///
/// The `crate_spec` must be in `name@version` format (e.g.
/// `"openssl-sys@0.9.102"`).  The `reason` is recorded for audit-trail
/// purposes.
pub fn run(crate_spec: &str, reason: &str) -> Result<()> {
    // -- 1. Parse crate spec -------------------------------------------------
    let (name, version) = parse_crate_spec(crate_spec)?;

    crate::validate::crate_name(&name)?;
    crate::validate::version(&version)?;

    // -- 2. Load or create safe-cargo.toml -----------------------------------
    let config_path = std::env::current_dir()?.join("safe-cargo.toml");

    let contents = if config_path.exists() {
        std::fs::read_to_string(&config_path).map_err(|e| Error::ConfigParse {
            path: config_path.clone(),
            reason: e.to_string(),
        })?
    } else {
        String::new()
    };

    let mut doc: toml::Value = if contents.is_empty() {
        toml::Value::Table(toml::Table::new())
    } else {
        toml::from_str(&contents)?
    };

    // -- 3. Add entry to [allow] section -------------------------------------
    let root = doc.as_table_mut().ok_or_else(|| Error::ConfigParse {
        path: config_path.clone(),
        reason: "config root is not a TOML table".to_string(),
    })?;

    // Ensure the [allow] table exists.
    if !root.contains_key("allow") {
        root.insert("allow".to_string(), toml::Value::Table(toml::Table::new()));
    }

    let allow_table = root
        .get_mut("allow")
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| Error::ConfigParse {
            path: config_path.clone(),
            reason: "[allow] is not a table".to_string(),
        })?;

    // Build the inline table for this entry:
    //   name = { version = "x.y.z", reason = "..." }
    let mut entry = toml::Table::new();
    entry.insert("version".to_string(), toml::Value::String(version.clone()));
    entry.insert(
        "reason".to_string(),
        toml::Value::String(reason.to_string()),
    );

    allow_table.insert(name.clone(), toml::Value::Table(entry));

    // -- 4. Write back to safe-cargo.toml ------------------------------------
    let serialized = toml::to_string_pretty(&doc).map_err(|e| Error::ConfigParse {
        path: config_path.clone(),
        reason: format!("failed to serialize config: {e}"),
    })?;

    let tmp_path = config_path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &serialized)?;
    if let Err(e) = std::fs::rename(&tmp_path, &config_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }

    // -- 5. Confirmation -----------------------------------------------------
    println!(
        "{} Added {}@{} to allowlist.",
        "✓".green(),
        name.bold(),
        version,
    );
    println!("  reason: {}", reason.dimmed());
    println!("  config: {}", config_path.display().to_string().dimmed());

    Ok(())
}

/// Parse a `name@version` spec, requiring that the version is present.
fn parse_crate_spec(spec: &str) -> Result<(String, String)> {
    let pos = spec.rfind('@').ok_or_else(|| Error::ConfigParse {
        path: "<crate_spec>".into(),
        reason: format!(
            "crate spec must be in name@version format (e.g. \"openssl-sys@0.9.102\"), got: {spec}"
        ),
    })?;

    let name = &spec[..pos];
    let version = &spec[pos + 1..];

    if name.is_empty() {
        return Err(Error::ConfigParse {
            path: "<crate_spec>".into(),
            reason: "crate name cannot be empty".to_string(),
        });
    }

    if version.is_empty() {
        return Err(Error::ConfigParse {
            path: "<crate_spec>".into(),
            reason: "version is required (e.g. \"openssl-sys@0.9.102\")".to_string(),
        });
    }

    Ok((name.to_string(), version.to_string()))
}
