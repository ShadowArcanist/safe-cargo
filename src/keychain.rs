use keyring::Entry;

use crate::error::{Error, Result};

const SERVICE_NAME: &str = "safe-cargo";

/// Store a GitHub token in the macOS Keychain for the given repository.
pub fn store_credentials(repo: &str, token: &str) -> Result<()> {
    let entry = build_entry(repo)?;
    entry.set_password(token).map_err(|e| match e {
        keyring::Error::NoEntry => Error::KeychainNotConfigured {
            repo: repo.to_string(),
        },
        keyring::Error::Ambiguous(_) | keyring::Error::PlatformFailure(_) => {
            Error::KeychainAccessDenied {
                repo: repo.to_string(),
                reason: e.to_string(),
            }
        }
        _ => Error::KeychainStoreFailed {
            repo: repo.to_string(),
            reason: e.to_string(),
        },
    })
}

/// Retrieve a previously stored GitHub token for the given repository.
pub fn get_token(repo: &str) -> Result<String> {
    let entry = build_entry(repo)?;
    entry.get_password().map_err(|e| match e {
        keyring::Error::NoEntry => Error::KeychainNotConfigured {
            repo: repo.to_string(),
        },
        keyring::Error::Ambiguous(_) | keyring::Error::PlatformFailure(_) => {
            Error::KeychainAccessDenied {
                repo: repo.to_string(),
                reason: e.to_string(),
            }
        }
        _ => Error::KeychainAccessDenied {
            repo: repo.to_string(),
            reason: e.to_string(),
        },
    })
}

#[allow(dead_code)]
pub fn delete_credentials(repo: &str) -> Result<()> {
    let entry = build_entry(repo)?;
    entry.delete_credential().map_err(|e| match e {
        keyring::Error::NoEntry => Error::KeychainNotConfigured {
            repo: repo.to_string(),
        },
        keyring::Error::Ambiguous(_) | keyring::Error::PlatformFailure(_) => {
            Error::KeychainAccessDenied {
                repo: repo.to_string(),
                reason: e.to_string(),
            }
        }
        _ => Error::KeychainDeleteFailed {
            repo: repo.to_string(),
            reason: e.to_string(),
        },
    })
}

/// Check whether credentials are stored for the given repository.
///
/// Returns `true` if a token exists, `false` if no entry is found.
/// Propagates other keychain errors.
#[allow(dead_code)]
pub fn has_credentials(repo: &str) -> Result<bool> {
    let entry = build_entry(repo)?;
    match entry.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(Error::KeychainAccessDenied {
            repo: repo.to_string(),
            reason: e.to_string(),
        }),
    }
}

/// Build a keyring `Entry` with our fixed service name and the repo as the
/// account identifier.
fn build_entry(repo: &str) -> Result<Entry> {
    Entry::new(SERVICE_NAME, repo).map_err(|e| Error::KeychainAccessDenied {
        repo: repo.to_string(),
        reason: e.to_string(),
    })
}
