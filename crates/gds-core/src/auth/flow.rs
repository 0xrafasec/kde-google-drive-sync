//! Full OAuth2 authorization flow and token provider (refresh + cache).

use std::sync::Arc;
use std::time::Duration;

use crate::auth::loopback::{bind_loopback, wait_for_redirect};
use crate::auth::oauth_client::ConfiguredBasicClient;
use crate::auth::oauth_client::{
    authorization_url, build_client, exchange_code, refresh_access_token, revoke_token,
    AuthUrlResult,
};
use crate::auth::token_store::TokenStore;
use crate::model::SyncError;
use chrono::Utc;

/// Time to wait for the user to complete the redirect.
const REDIRECT_TIMEOUT: Duration = Duration::from_secs(300); // 5 min

/// Buffer before expiry to refresh (refresh if token expires in less than this).
const REFRESH_BUFFER_SECS: i64 = 60;

/// Runs the full authorization flow: bind loopback, build client, get auth URL,
/// optionally open browser, wait for redirect, exchange code, store refresh token.
///
/// `open_url`: if provided, called with the auth URL (e.g. to run `xdg-open`).
/// If None or if it fails, returns `Err(SyncError::OpenUrlRequired { url })` so the caller can show the URL.
pub async fn authorize_flow(
    client_id: &str,
    client_secret: Option<&str>,
    preferred_redirect_port: u16,
    store: &dyn TokenStore,
    account_key: &str,
    open_url: Option<impl FnOnce(&str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>>,
) -> Result<(), SyncError> {
    let (listener, actual_port) = bind_loopback(preferred_redirect_port).await?;
    let client = build_client(client_id, client_secret, actual_port)?;
    let AuthUrlResult {
        url,
        csrf_state,
        pkce_verifier,
    } = authorization_url(&client)?;

    if let Some(open) = open_url {
        if open(&url).is_err() {
            return Err(SyncError::OpenUrlRequired { url });
        }
    } else {
        return Err(SyncError::OpenUrlRequired { url });
    }

    let (code, state) = wait_for_redirect(listener, REDIRECT_TIMEOUT).await?;

    if state != *csrf_state.secret() {
        return Err(SyncError::AuthError {
            message: "CSRF state mismatch".to_string(),
        });
    }

    let result = exchange_code(&client, &code, pkce_verifier).await?;
    store.store_refresh_token(account_key, &result.refresh_token)?;
    Ok(())
}

/// Provides valid access tokens by refreshing when needed. Thread-safe.
pub struct TokenProvider {
    client: ConfiguredBasicClient,
    store: Arc<dyn TokenStore>,
    /// (access_token, expires_at unix timestamp)
    cache: tokio::sync::RwLock<std::collections::HashMap<String, (String, i64)>>,
}

impl TokenProvider {
    pub fn new(
        client_id: &str,
        client_secret: Option<&str>,
        redirect_port: u16,
        store: Arc<dyn TokenStore>,
    ) -> Result<Self, SyncError> {
        let client = build_client(client_id, client_secret, redirect_port)?;
        Ok(Self {
            client,
            store,
            cache: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        })
    }

    /// Creates a provider with a pre-built client (e.g. for tests with mock token URL).
    pub fn with_client(client: ConfiguredBasicClient, store: Arc<dyn TokenStore>) -> Self {
        Self {
            client,
            store,
            cache: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Returns a valid access token for the account, refreshing if necessary.
    pub async fn get_valid_access_token(&self, account_key: &str) -> Result<String, SyncError> {
        let now = Utc::now().timestamp();
        {
            let cache = self.cache.read().await;
            if let Some((token, expires_at)) = cache.get(account_key) {
                if *expires_at > now + REFRESH_BUFFER_SECS {
                    return Ok(token.clone());
                }
            }
        }

        let refresh = self
            .store
            .get_refresh_token(account_key)?
            .ok_or(SyncError::AuthExpired)?;

        let result = refresh_access_token(&self.client, &refresh).await?;
        let expires_at = now + result.expires_in.as_secs() as i64;
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                account_key.to_string(),
                (result.access_token.clone(), expires_at),
            );
        }
        Ok(result.access_token)
    }

    /// Revokes the refresh token at Google and removes it from the store.
    pub async fn revoke_and_remove(&self, account_key: &str) -> Result<(), SyncError> {
        if let Some(refresh) = self.store.get_refresh_token(account_key)? {
            let _ = revoke_token(&refresh).await; // best-effort
        }
        self.store.delete_refresh_token(account_key)?;
        self.cache.write().await.remove(account_key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{build_client_with_urls, InMemoryTokenStore};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_token_store_in_memory() {
        let store = InMemoryTokenStore::new();
        store.store_refresh_token("acc1", "refresh_abc").unwrap();
        assert_eq!(
            store.get_refresh_token("acc1").unwrap(),
            Some("refresh_abc".to_string())
        );
        assert!(store.get_refresh_token("acc2").unwrap().is_none());
        store.delete_refresh_token("acc1").unwrap();
        assert!(store.get_refresh_token("acc1").unwrap().is_none());
    }

    #[tokio::test]
    async fn test_exchange_code_with_mock_server() {
        let mock = MockServer::start().await;
        let token_response = serde_json::json!({
            "access_token": "at_123",
            "refresh_token": "rt_456",
            "expires_in": 3600,
            "token_type": "Bearer"
        });
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token_response))
            .mount(&mock)
            .await;

        let redirect = "http://127.0.0.1:9876/";
        let client = build_client_with_urls(
            "test_client_id",
            Some("test_secret"),
            redirect,
            &format!("{}/auth", mock.uri()),
            &format!("{}/token", mock.uri()),
        )
        .unwrap();

        let (_pkce_challenge, pkce_verifier) = oauth2::PkceCodeChallenge::new_random_sha256();
        let result = exchange_code(&client, "test_auth_code", pkce_verifier)
            .await
            .unwrap();
        assert_eq!(result.access_token, "at_123");
        assert_eq!(result.refresh_token, "rt_456");
        assert_eq!(result.expires_in.as_secs(), 3600);
    }

    #[tokio::test]
    async fn test_token_provider_refresh_and_cache() {
        let mock = MockServer::start().await;
        let token_response = serde_json::json!({
            "access_token": "at_cached",
            "expires_in": 3600,
            "token_type": "Bearer"
        });
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token_response))
            .mount(&mock)
            .await;

        let store = Arc::new(InMemoryTokenStore::new());
        store.store_refresh_token("acc1", "rt_xyz").unwrap();

        let client = build_client_with_urls(
            "cid",
            None,
            "http://127.0.0.1:9999/",
            &format!("{}/auth", mock.uri()),
            &format!("{}/token", mock.uri()),
        )
        .unwrap();

        let provider = TokenProvider::with_client(client, store.clone());

        let t1 = provider.get_valid_access_token("acc1").await.unwrap();
        assert_eq!(t1, "at_cached");
        let t2 = provider.get_valid_access_token("acc1").await.unwrap();
        assert_eq!(t2, "at_cached"); // from cache
    }
}
