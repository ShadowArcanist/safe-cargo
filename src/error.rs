use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // Config errors
    #[error("config file not found (searched up to root and ~/.config/safe-cargo/)")]
    ConfigNotFound,

    #[error("failed to parse config at {path}: {reason}")]
    ConfigParse { path: PathBuf, reason: String },

    // Keychain errors
    #[error("no credentials configured for repo '{repo}'")]
    KeychainNotConfigured { repo: String },

    #[error("keychain access denied for repo '{repo}': {reason}")]
    KeychainAccessDenied { repo: String, reason: String },

    #[error("failed to store credentials for repo '{repo}': {reason}")]
    KeychainStoreFailed { repo: String, reason: String },

    #[allow(dead_code)]
    #[error("failed to delete credentials for repo '{repo}': {reason}")]
    KeychainDeleteFailed { repo: String, reason: String },

    // Network errors
    #[error("Network error: {0}")]
    Network(String),

    #[error("crates.io API error: {0}")]
    CratesIoApi(String),

    #[error("GitHub API error: {0}")]
    GitHubApi(String),

    // Report errors
    #[error("report not found for crate '{name}' v{version}")]
    ReportNotFound { name: String, version: String },

    #[error("failed to parse report for crate '{name}': {reason}")]
    ReportParse { name: String, reason: String },

    // Policy errors
    #[allow(dead_code)]
    #[error("risk score {score} exceeds maximum allowed {max} for crate '{name}'")]
    ScoreTooHigh { name: String, score: u32, max: u32 },

    #[error("unanalyzed dependencies: {crates:?}")]
    UnanalyzedDeps { crates: Vec<String> },

    // Cargo errors
    #[error("cargo command failed: {0}")]
    CargoCommandFailed(String),

    #[error("cargo is not installed or not in PATH")]
    CargoNotInstalled,

    // Signing errors
    #[error("signature verification is required but no signing key is configured; run `safe-cargo setup` to configure a signing key")]
    SigningRequired,

    #[error(
        "Report integrity failure for {0}@{1}: crate name or version mismatch (possible tampering)"
    )]
    ReportIntegrityFailure(String, String),

    #[error("Response too large from {0} (limit: {1} bytes)")]
    ResponseTooLarge(String, usize),

    #[allow(dead_code)]
    #[error(
        "Blocked passthrough command '{0}': run `safe-cargo check` first to verify dependencies"
    )]
    BlockedPassthrough(String),

    // Input validation
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    // General IO
    #[error(transparent)]
    Io(#[from] std::io::Error),

    // Serialization
    #[error(transparent)]
    TomlDeserialize(#[from] toml::de::Error),
}
