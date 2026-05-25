use crate::error::{Error, Result};

/// Validate a crate name against the crates.io package name grammar.
/// Must be 1-64 chars, start with a letter, contain only [a-zA-Z0-9_-].
pub fn crate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        return Err(Error::ConfigParse {
            path: "<input>".into(),
            reason: format!("crate name must be 1-64 characters, got: '{name}'"),
        });
    }
    let first = name.chars().next().unwrap();
    if !first.is_ascii_alphabetic() {
        return Err(Error::ConfigParse {
            path: "<input>".into(),
            reason: format!("crate name must start with a letter, got: '{name}'"),
        });
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(Error::ConfigParse {
            path: "<input>".into(),
            reason: format!("crate name contains invalid characters: '{name}'"),
        });
    }
    Ok(())
}

/// Validate a version string as semver (e.g. "1.2.3", "1.2.3-beta.1").
pub fn version(ver: &str) -> Result<()> {
    semver::Version::parse(ver).map_err(|e| Error::ConfigParse {
        path: "<input>".into(),
        reason: format!("invalid version '{ver}': {e}"),
    })?;
    Ok(())
}

/// Validate a GitHub repo string is in "owner/name" format with safe characters.
pub fn github_repo(repo: &str) -> Result<()> {
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(Error::ConfigParse {
            path: "<input>".into(),
            reason: format!("repo must be in owner/name format, got: '{repo}'"),
        });
    }
    for part in &parts {
        if !part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(Error::ConfigParse {
                path: "<input>".into(),
                reason: format!("repo contains invalid characters: '{repo}'"),
            });
        }
    }
    Ok(())
}

/// Validate a git branch name (no spaces, no .., no special chars that could be injected).
pub fn branch_name(branch: &str) -> Result<()> {
    if branch.is_empty() {
        return Err(Error::ConfigParse {
            path: "<input>".into(),
            reason: "branch name cannot be empty".into(),
        });
    }
    if branch.contains("..")
        || branch.contains(' ')
        || branch.contains('~')
        || branch.contains('^')
        || branch.contains(':')
        || branch.contains('\\')
        || branch.starts_with('-')
        || branch.ends_with('/')
        || branch.ends_with(".lock")
    {
        return Err(Error::ConfigParse {
            path: "<input>".into(),
            reason: format!("branch name contains invalid characters: '{branch}'"),
        });
    }
    Ok(())
}

/// Validate a string for use in filesystem paths.
/// Only allows `[a-zA-Z0-9._-+]`, rejects empty strings and path traversal.
/// The `+` is needed for semver build metadata (e.g. `1.1.2+spec-1.1.0`).
pub fn safe_path_component(name: &str) -> Result<String> {
    if name.is_empty() {
        return Err(Error::InvalidInput("path component cannot be empty".into()));
    }
    if name == ".." || name == "." {
        return Err(Error::InvalidInput("path traversal not allowed".into()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '+')
    {
        return Err(Error::InvalidInput(format!(
            "invalid characters in path component: {}",
            name
        )));
    }
    Ok(name.to_string())
}

/// Normalize a crate name by replacing hyphens with underscores,
/// matching Cargo's canonical form.
#[allow(dead_code)]
pub fn normalize_crate_name(name: &str) -> String {
    name.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_crate_names() {
        assert!(crate_name("tokio").is_ok());
        assert!(crate_name("serde_json").is_ok());
        assert!(crate_name("my-crate").is_ok());
    }

    #[test]
    fn invalid_crate_names() {
        assert!(crate_name("").is_err());
        assert!(crate_name("123abc").is_err());
        assert!(crate_name("my crate").is_err());
        assert!(crate_name("../evil").is_err());
        assert!(crate_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn valid_versions() {
        assert!(version("1.0.0").is_ok());
        assert!(version("0.1.0-beta.1").is_ok());
    }

    #[test]
    fn invalid_versions() {
        assert!(version("not-a-version").is_err());
        assert!(version("").is_err());
    }

    #[test]
    fn valid_repos() {
        assert!(github_repo("user/repo").is_ok());
        assert!(github_repo("my-org/my-repo").is_ok());
    }

    #[test]
    fn invalid_repos() {
        assert!(github_repo("noslash").is_err());
        assert!(github_repo("").is_err());
        assert!(github_repo("a/b/c").is_err());
    }

    #[test]
    fn safe_paths() {
        assert!(safe_path_component("tokio").is_ok());
        assert!(safe_path_component("../etc/passwd").is_err());
        assert!(safe_path_component("/root").is_err());
    }

    #[test]
    fn valid_branches() {
        assert!(branch_name("main").is_ok());
        assert!(branch_name("feature/foo").is_ok());
    }

    #[test]
    fn invalid_branches() {
        assert!(branch_name("").is_err());
        assert!(branch_name("a..b").is_err());
        assert!(branch_name("-starts-with-dash").is_err());
    }
}
