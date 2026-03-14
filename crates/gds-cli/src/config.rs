//! Config and data paths (same semantics as daemon). Configure writes OAuth credentials to the DB.

use std::path::PathBuf;


/// Config directory (same semantics as daemon: GDS_CONFIG_DIR or ~/.config/gds).
pub fn config_dir() -> PathBuf {
    std::env::var("GDS_CONFIG_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs::config_dir().map(|d| d.join("gds")))
        .unwrap_or_else(|| PathBuf::from(".config").join("gds"))
}

/// Data directory (same semantics as daemon: GDS_DATA_DIR or ~/.local/share/gds). DB lives here.
pub fn data_dir() -> PathBuf {
    std::env::var("GDS_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|d| d.join("gds")))
        .unwrap_or_else(|| PathBuf::from(".local").join("share").join("gds"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_and_data_dir_resolve() {
        let _ = config_dir();
        let _ = data_dir();
    }
}
