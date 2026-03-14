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
            let msg = e.to_string();
            if msg.contains("No such")
                || msg.contains("not found")
                || msg.contains("No matching entry")
                || msg.contains("secure storage")
            {
                Ok(None)
            } else {
                tracing::debug!(
                    key = %key,
                    error = %msg,
                    "keyring get_password failed (e.g. keyring locked or unavailable)"
                );
                Err(SyncError::AuthError { message: msg })
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

    fn get_oauth_client_secret(&self) -> Result<Option<String>, SyncError> {
        self.get_refresh_token(crate::auth::token_store::OAUTH_CLIENT_SECRET_KEY)
    }

    fn set_oauth_client_secret(&self, secret: &str) -> Result<(), SyncError> {
        self.store_refresh_token(
            crate::auth::token_store::OAUTH_CLIENT_SECRET_KEY,
            secret,
        )
    }
}
