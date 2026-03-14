//! Application configuration (serde + Default). No secrets.

use serde::{Deserialize, Serialize};

/// OAuth / auth section. Prefer `credentials_path` to a Google JSON file (client_id + secret);
/// otherwise set `client_id` and leave secret out (PKCE-only).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthConfig {
    #[serde(rename = "client_id")]
    pub client_id: String,
    /// Optional path to Google OAuth JSON (Desktop app). Relative to config dir or absolute.
    /// File must stay outside the repo; see README and docs/GOOGLE_API.md.
    #[serde(rename = "credentials_path", default)]
    pub credentials_path: Option<String>,
    #[serde(rename = "redirect_port")]
    pub redirect_port: u16,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            credentials_path: None,
            redirect_port: 8765,
        }
    }
}

/// Sync behavior and limits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(rename = "poll_interval_secs")]
    pub poll_interval_secs: u32,
    #[serde(rename = "max_concurrent_uploads")]
    pub max_concurrent_uploads: u32,
    #[serde(rename = "max_concurrent_downloads")]
    pub max_concurrent_downloads: u32,
    #[serde(rename = "conflict_suffix_format")]
    pub conflict_suffix_format: String,
    /// Default request timeout (seconds). Used for metadata and small transfers.
    #[serde(rename = "request_timeout_secs")]
    pub request_timeout_secs: u64,
    /// Timeout for large uploads/downloads (seconds).
    #[serde(rename = "upload_timeout_secs")]
    pub upload_timeout_secs: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30,
            max_concurrent_uploads: 2,
            max_concurrent_downloads: 4,
            conflict_suffix_format: ".conflict-%Y%m%d-%H%M%S".to_string(),
            request_timeout_secs: 30,
            upload_timeout_secs: 300,
        }
    }
}

/// UI / notifications.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(rename = "show_notifications")]
    pub show_notifications: bool,
    #[serde(rename = "notification_timeout_ms")]
    pub notification_timeout_ms: u32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_notifications: true,
            notification_timeout_ms: 5000,
        }
    }
}

/// Root config (all configurable values). Load from TOML.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub oauth: OAuthConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            oauth: OAuthConfig::default(),
            sync: SyncConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_values() {
        let c = Config::default();
        assert_eq!(c.oauth.redirect_port, 8765);
        assert_eq!(c.sync.poll_interval_secs, 30);
        assert_eq!(c.sync.max_concurrent_uploads, 2);
        assert_eq!(c.sync.max_concurrent_downloads, 4);
        assert_eq!(c.sync.request_timeout_secs, 30);
        assert_eq!(c.sync.upload_timeout_secs, 300);
        assert!(c.ui.show_notifications);
        assert_eq!(c.ui.notification_timeout_ms, 5000);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let c = Config::default();
        let json = serde_json::to_string(&c).unwrap();
        let c2: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(c.oauth.redirect_port, c2.oauth.redirect_port);
        assert_eq!(
            c.sync.conflict_suffix_format,
            c2.sync.conflict_suffix_format
        );
    }
}
