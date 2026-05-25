use std::io::{self, Write};

use colored::Colorize;

use crate::error::Result;
use crate::keychain;

/// Interactive setup command.
///
/// Prompts the user for their GitHub repository, branch, and personal access
/// token, then persists the token in the macOS Keychain and writes a
/// `safe-cargo.toml` configuration file in the current directory.
pub fn run() -> Result<()> {
    let stdin = io::stdin();

    // -- 1. GitHub repository ------------------------------------------------
    print!("GitHub repo (owner/name, e.g. ShadowArcanist/safe-cargo): ");
    io::stdout().flush()?;

    let mut repo = String::new();
    stdin.read_line(&mut repo)?;
    let repo = repo.trim().to_string();

    crate::validate::github_repo(&repo)?;

    // -- 2. Branch -----------------------------------------------------------
    print!("Branch [main]: ");
    io::stdout().flush()?;

    let mut branch = String::new();
    stdin.read_line(&mut branch)?;
    let branch = branch.trim().to_string();
    let branch = if branch.is_empty() {
        "main".to_string()
    } else {
        branch
    };

    crate::validate::branch_name(&branch)?;

    // -- 3. GitHub token -----------------------------------------------------
    print!("GitHub personal access token: ");
    io::stdout().flush()?;

    let token = rpassword::read_password().map_err(|e| {
        crate::error::Error::CargoCommandFailed(format!("failed to read token: {e}"))
    })?;
    let token = token.trim().to_string();

    if token.is_empty() {
        eprintln!("Error: a GitHub token is required for API access");
        std::process::exit(1);
    }

    // -- 4. Signing key ------------------------------------------------------
    print!("Signing public key (required, minisign RW... key): ");
    io::stdout().flush()?;

    let mut signing_key = String::new();
    stdin.read_line(&mut signing_key)?;
    let signing_key = signing_key.trim().to_string();
    if signing_key.is_empty() {
        eprintln!(
            "{}",
            "Error: a signing public key is required for report verification.".red()
        );
        std::process::exit(1);
    }

    if minisign_verify::PublicKey::from_base64(&signing_key).is_err() {
        eprintln!(
            "{}",
            "Error: invalid signing key format. Must be a valid minisign public key.".red()
        );
        std::process::exit(1);
    }

    // -- 5. Store token in Keychain ------------------------------------------
    keychain::store_credentials(&repo, &token)?;

    // -- 6. Write safe-cargo.toml --------------------------------------------
    let mut config_table = toml::Table::new();
    let mut registry = toml::Table::new();
    registry.insert("repo".to_string(), toml::Value::String(repo.clone()));
    registry.insert("branch".to_string(), toml::Value::String(branch.clone()));
    registry.insert(
        "signing_key".to_string(),
        toml::Value::String(signing_key.clone()),
    );
    config_table.insert("registry".to_string(), toml::Value::Table(registry));

    let mut policy_table = toml::Table::new();
    policy_table.insert(
        "min_release_age".to_string(),
        toml::Value::String("3d".to_string()),
    );
    policy_table.insert("max_score".to_string(), toml::Value::Integer(40));
    policy_table.insert("require_report".to_string(), toml::Value::Boolean(true));
    config_table.insert("policy".to_string(), toml::Value::Table(policy_table));

    let config_contents =
        toml::to_string_pretty(&toml::Value::Table(config_table)).map_err(|e| {
            crate::error::Error::ConfigParse {
                path: "safe-cargo.toml".into(),
                reason: format!("failed to serialize config: {e}"),
            }
        })?;

    let config_path = std::env::current_dir()?.join("safe-cargo.toml");
    let tmp_path = config_path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &config_contents)?;
    if let Err(e) = std::fs::rename(&tmp_path, &config_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }

    // -- 7. Success ----------------------------------------------------------
    println!();
    println!("Setup complete!");
    println!("  - Token stored in macOS Keychain for repo \"{repo}\"");
    println!("  - Config written to ./safe-cargo.toml");
    println!();
    println!("You can now use `safe-cargo add <crate>` to add dependencies.");

    Ok(())
}
