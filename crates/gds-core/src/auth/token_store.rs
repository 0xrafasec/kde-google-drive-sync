//! Token storage abstraction. Implementations: InMemory (tests); libsecret/KWallet in daemon.

use crate::model::SyncError;

/// Storage for OAuth refresh tokens. Implementations must never persist tokens in plain files.
pub trait TokenStore: Send + Sync {
    /// Store the refresh token for the given key (e.g. account id).
    fn store_refresh_token(&self, key: &str, token: &str) -> Result<(), SyncError>;

    /// Load the refresh token for the given key.
    fn get_refresh_token(&self, key: &str) -> Result<Option<String>, SyncError>;

    /// Remove the refresh token (e.g. on account removal or revocation).
    fn delete_refresh_token(&self, key: &str) -> Result<(), SyncError>;
}

/// In-memory token store for tests only. SECURITY: never use in production.
#[derive(Default)]
pub struct InMemoryTokenStore {
    tokens: std::sync::RwLock<std::collections::HashMap<String, String>>,
}

impl InMemoryTokenStore {
    pub fn new() -> Self {
        Self {
            tokens: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
}

impl TokenStore for InMemoryTokenStore {
    fn store_refresh_token(&self, key: &str, token: &str) -> Result<(), SyncError> {
        self.tokens
            .write()
            .map_err(|e| SyncError::AuthError {
                message: e.to_string(),
            })?
            .insert(key.to_string(), token.to_string());
        Ok(())
    }

    fn get_refresh_token(&self, key: &str) -> Result<Option<String>, SyncError> {
        Ok(self
            .tokens
            .read()
            .map_err(|e| SyncError::AuthError {
                message: e.to_string(),
            })?
            .get(key)
            .cloned())
    }

    fn delete_refresh_token(&self, key: &str) -> Result<(), SyncError> {
        self.tokens
            .write()
            .map_err(|e| SyncError::AuthError {
                message: e.to_string(),
            })?
            .remove(key);
        Ok(())
    }
}
