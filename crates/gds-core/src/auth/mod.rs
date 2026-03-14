//! Authentication: OAuth2 (PKCE), token store, loopback redirect, refresh.

mod flow;
#[cfg(feature = "keyring")]
mod keyring_store;
mod loopback;
mod oauth_client;
mod token_store;

pub use flow::{authorize_flow, TokenProvider};
pub use loopback::{bind_loopback, parse_redirect_query, wait_for_redirect};
pub use oauth_client::{
    build_client, build_client_with_urls, revoke_token, AuthUrlResult, ExchangeResult,
    RefreshResult, SCOPE_DRIVE, SCOPE_EMAIL,
};
pub use token_store::{InMemoryTokenStore, TokenStore};

#[cfg(feature = "keyring")]
pub use keyring_store::KeyringTokenStore;
