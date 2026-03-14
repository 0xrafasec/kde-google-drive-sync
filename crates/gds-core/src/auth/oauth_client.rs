//! OAuth2 client for Google (PKCE, auth URL, token exchange, refresh, revoke).

use std::time::Duration;

use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

use crate::model::SyncError;

/// OAuth2 client with auth and token endpoints set (required for authorize_url and exchange_code).
pub type ConfiguredBasicClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

/// Google OAuth2 endpoints.
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";

/// Required scopes for Drive sync and user email.
pub const SCOPE_DRIVE: &str = "https://www.googleapis.com/auth/drive";
pub const SCOPE_EMAIL: &str = "https://www.googleapis.com/auth/userinfo.email";

/// Builds a Google OAuth2 client with PKCE (no client secret required for public flow,
/// but Google desktop apps typically use client secret for token exchange).
pub fn build_client(
    client_id: &str,
    client_secret: Option<&str>,
    redirect_port: u16,
) -> Result<ConfiguredBasicClient, SyncError> {
    let redirect_url = format!("http://127.0.0.1:{}/", redirect_port);
    build_client_with_urls(client_id, client_secret, &redirect_url, AUTH_URL, TOKEN_URL)
}

/// Builds an OAuth2 client with custom URLs (for testing with mock servers).
/// oauth2 v5: typestate builder — set_auth_uri and set_token_uri are required for authorize_url and exchange_code.
pub fn build_client_with_urls(
    client_id: &str,
    client_secret: Option<&str>,
    redirect_url: &str,
    auth_url: &str,
    token_url: &str,
) -> Result<ConfiguredBasicClient, SyncError> {
    let auth_url = AuthUrl::new(auth_url.to_string()).map_err(|e| SyncError::AuthError {
        message: e.to_string(),
    })?;
    let token_url = TokenUrl::new(token_url.to_string()).map_err(|e| SyncError::AuthError {
        message: e.to_string(),
    })?;
    let redirect_url =
        RedirectUrl::new(redirect_url.to_string()).map_err(|e| SyncError::AuthError {
            message: e.to_string(),
        })?;

    let client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url);

    #[allow(non_snake_case)]
    let client = match client_secret {
        Some(s) => client.set_client_secret(ClientSecret::new(s.to_string())),
        None => client,
    };
    Ok(client)
}

/// Result of building an authorization URL: URL to open in browser, and state/verifier for later.
pub struct AuthUrlResult {
    pub url: String,
    pub csrf_state: CsrfToken,
    pub pkce_verifier: PkceCodeVerifier,
}

/// Generates PKCE challenge and returns the authorization URL plus state and verifier.
pub fn authorization_url(client: &ConfiguredBasicClient) -> Result<AuthUrlResult, SyncError> {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(SCOPE_DRIVE.to_string()))
        .add_scope(Scope::new(SCOPE_EMAIL.to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();
    Ok(AuthUrlResult {
        url: auth_url.to_string(),
        csrf_state,
        pkce_verifier,
    })
}

/// Exchanges an authorization code for tokens. Returns refresh_token and access_token with expiry.
pub async fn exchange_code(
    client: &ConfiguredBasicClient,
    code: &str,
    pkce_verifier: PkceCodeVerifier,
) -> Result<ExchangeResult, SyncError> {
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| SyncError::AuthError {
            message: e.to_string(),
        })?;

    let token_result = client
        .exchange_code(AuthorizationCode::new(code.to_string()))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client)
        .await
        .map_err(|e: oauth2::RequestTokenError<_, _>| SyncError::AuthError {
            message: e.to_string(),
        })?;

    let refresh_token = token_result
        .refresh_token()
        .map(|t| t.secret().to_string())
        .ok_or_else(|| SyncError::AuthError {
            message: "No refresh token in response".to_string(),
        })?;

    let access_token = token_result.access_token().secret().to_string();
    let expires_in = token_result
        .expires_in()
        .unwrap_or(Duration::from_secs(3600));

    Ok(ExchangeResult {
        refresh_token,
        access_token,
        expires_in,
    })
}

pub struct ExchangeResult {
    pub refresh_token: String,
    pub access_token: String,
    pub expires_in: Duration,
}

/// Refreshes the access token using the refresh token.
pub async fn refresh_access_token(
    client: &ConfiguredBasicClient,
    refresh_token: &str,
) -> Result<RefreshResult, SyncError> {
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| SyncError::AuthError {
            message: e.to_string(),
        })?;

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
        .request_async(&http_client)
        .await
        .map_err(|e: oauth2::RequestTokenError<_, _>| SyncError::AuthError {
            message: e.to_string(),
        })?;

    let access_token = token_result.access_token().secret().to_string();
    let expires_in = token_result
        .expires_in()
        .unwrap_or(Duration::from_secs(3600));

    Ok(RefreshResult {
        access_token,
        expires_in,
    })
}

pub struct RefreshResult {
    pub access_token: String,
    pub expires_in: Duration,
}

/// Revokes the refresh token at Google (call on account removal).
pub async fn revoke_token(token: &str) -> Result<(), SyncError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| SyncError::NetworkError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    let res = client
        .post(REVOKE_URL)
        .form(&[("token", token)])
        .send()
        .await
        .map_err(|e| SyncError::NetworkError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    // Google returns 200 even if token was already revoked or invalid.
    if res.status().is_client_error() || res.status().is_server_error() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(SyncError::AuthError {
            message: format!("Revoke failed {}: {}", status, body),
        });
    }
    Ok(())
}
