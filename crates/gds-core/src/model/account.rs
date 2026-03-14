//! Account model — linked Google account and keyring reference.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A linked Google account (OAuth identity + keyring key for tokens).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub email: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    /// Keyring entry key for refresh_token (e.g. service "gds", key = account id).
    #[serde(rename = "keyringKey")]
    pub keyring_key: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_account() -> Account {
        Account {
            id: "acc-1".to_string(),
            email: "user@gmail.com".to_string(),
            display_name: Some("User".to_string()),
            keyring_key: "gds:acc-1".to_string(),
            created_at: DateTime::<Utc>::from_timestamp_secs(1_700_000_000)
                .unwrap_or_else(Utc::now),
        }
    }

    #[test]
    fn account_serialization_roundtrip() {
        let a = sample_account();
        let json = serde_json::to_string(&a).unwrap();
        let a2: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(a, a2);
    }
}
