use crate::error::{Error, Result};

/// Verify a minisign signature against the given public key and data.
/// Returns Ok(()) if valid, Err if invalid or missing key.
pub fn verify(public_key_str: &str, data: &[u8], signature_str: &str) -> Result<()> {
    let pk = minisign_verify::PublicKey::from_base64(public_key_str).map_err(|e| {
        Error::ConfigParse {
            path: "<signing_key>".into(),
            reason: format!("invalid signing public key: {e}"),
        }
    })?;

    let sig =
        minisign_verify::Signature::decode(signature_str).map_err(|e| Error::ReportParse {
            name: "<signature>".into(),
            reason: format!("invalid signature format: {e}"),
        })?;

    pk.verify(data, &sig, false)
        .map_err(|e| Error::ReportParse {
            name: "<signature>".into(),
            reason: format!("signature verification failed: {e}"),
        })
}
