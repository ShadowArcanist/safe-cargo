use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "safe-cargo",
    about = "A cargo wrapper resistant to supply chain attacks",
    version,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Configure safe-cargo (repo URL, branch, GitHub token stored in keychain)
    Setup,

    /// Add a crate after verifying it passes security analysis
    Add {
        /// Crate name, optionally with @version (e.g. "tokio" or "tokio@1.38.0")
        crate_spec: String,
    },

    /// Verify all dependencies in Cargo.lock have passing reports
    Check,

    /// Show analysis status for all dependencies
    Status,

    /// Manually allow a crate version, bypassing analysis
    Allow {
        /// Crate spec in name@version format (e.g. "openssl-sys@0.9.102")
        crate_spec: String,

        /// Reason for allowing (required for audit trail)
        #[arg(long)]
        reason: String,
    },

    /// Re-trigger analysis for all dependencies
    Audit,

    /// Run cargo update, then verify new versions
    Update,

    /// Pass-through: any unrecognized command is forwarded to cargo
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

/// Parse "crate@version" or "crate" into (name, Option<version>).
pub fn parse_crate_spec(spec: &str) -> (String, Option<String>) {
    if let Some(pos) = spec.rfind('@') {
        let name = spec[..pos].to_string();
        let version = spec[pos + 1..].to_string();
        if version.is_empty() {
            (name, None)
        } else {
            (name, Some(version))
        }
    } else {
        (spec.to_string(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_crate_spec_name_only() {
        let (name, ver) = parse_crate_spec("tokio");
        assert_eq!(name, "tokio");
        assert_eq!(ver, None);
    }

    #[test]
    fn parse_crate_spec_with_version() {
        let (name, ver) = parse_crate_spec("tokio@1.38.0");
        assert_eq!(name, "tokio");
        assert_eq!(ver, Some("1.38.0".to_string()));
    }

    #[test]
    fn parse_crate_spec_trailing_at() {
        let (name, ver) = parse_crate_spec("tokio@");
        assert_eq!(name, "tokio");
        assert_eq!(ver, None);
    }
}
