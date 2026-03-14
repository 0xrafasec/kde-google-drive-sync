//! TokenStore implementation using the keyring crate (libsecret on Linux).
//! Enabled with the `keyring` feature. SECURITY: tokens are never written to disk by this code.

use crate::auth::token_store::TokenStore;
use crate::model::SyncError;

const KEYRING_SERVICE: &str = "gds";

/// Token store backed by the system keyring (libsecret on Linux, etc.).
#[cfg(feature = "keyring")]
pub struct KeyringTokenStore;

#[cfg(feature = "keyring")]
impl TokenStore for KeyringTokenStore {
    fn store_refresh_token(&self, key: &str, token: &str) -> Result<(), SyncError> {
        let entry =
            keyring::Entry::new(KEYRING_SERVICE, key).map_err(|e| SyncError::AuthError {
                message: e.to_string(),
            })?;
        entry
            .set_password(token)
            .map_err(|e| SyncError::AuthError {
                message: e.to_string(),
            })?;
        Ok(())
    }

    fn get_refresh_token(&self, key: &str) -> Result<Option<String>, SyncError> {
        let entry =
            keyring::Entry::new(KEYRING_SERVICE, key).map_err(|e| SyncError::AuthError {
                message: e.to_string(),
            })?;
        entry.get_password().map(Some).or_else(|e| {
            if e.to_string().contains("No such") || e.to_string().contains("not found") {
                Ok(None)
            } else {
                Err(SyncError::AuthError {
                    message: e.to_string(),
                })
            }
        })
    }

    fn delete_refresh_token(&self, key: &str) -> Result<(), SyncError> {
        let entry =
            keyring::Entry::new(KEYRING_SERVICE, key).map_err(|e| SyncError::AuthError {
                message: e.to_string(),
            })?;
        let _ = entry.delete_credential(); // idempotent: ignore if already missing
        Ok(())
    }
}
