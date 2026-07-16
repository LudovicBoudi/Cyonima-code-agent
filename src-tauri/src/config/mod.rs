//! Configuration globale + par-projet.
//!
//! Cf `docs/ARCHITECTURE.md` :
//! - globale : `~/.cyonima/config.toml`
//! - override par projet : `<workspace>/.cyonima/config.toml`
//! - API keys : keyring OS (jamais en clair)
//!
//! Le parsing concret arrive en J9. Squelette J0.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub storage: StorageConfig,
    pub permissions: PermissionsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub models_dir: PathBuf,
}

impl Default for StorageConfig {
    fn default() -> Self {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            models_dir: base.join(".cyonima").join("models"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionsConfig {
    /// Map tool name → policy. Les outils absents retombent sur [`crate::permissions::default_policy`].
    pub overrides: std::collections::HashMap<String, String>,
}
