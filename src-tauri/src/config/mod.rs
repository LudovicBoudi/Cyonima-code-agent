//! Configuration globale + par-projet.
//!
//! Cf `docs/ARCHITECTURE.md` :
//! - globale : `~/.cyonima/config.toml`
//! - override par projet : `<workspace>/.cyonima/config.toml`
//! - API keys : keyring OS (jamais en clair)
//!
//! La config par projet surcharge la config globale. Les champs non
//! spécifiés dans le projet tombent sur les valeurs globales (qui
//! tombent elles-mêmes sur les defaults codés en dur).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Config globale + per-workspace, chargée une fois au démarrage et
/// écrite à la demande via IPC `config_set_*`.
#[derive(Clone)]
pub struct ConfigManager {
    inner: Arc<RwLock<Config>>,
    global_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub permissions: PermissionsConfig,
    #[serde(default)]
    pub provider: ProviderConfig,
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
    /// Map tool name → policy ("auto" | "ask" | "deny"). Les outils absents
    /// retombent sur les defaults du module `permissions`.
    pub overrides: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    /// Provider utilisé par défaut pour les nouvelles sessions (ex: "ollama", "openai").
    /// None = pas de override, le choix est laissé à l'utilisateur dans l'UI.
    pub default_provider: Option<String>,
    /// Modèle par défaut (ex: "gemma4:12b", "gpt-4o").
    pub default_model: Option<String>,
    /// Endpoint Ollama override (défaut: http://localhost:11434).
    pub ollama_endpoint: Option<String>,
}

impl ConfigManager {
    /// Chemin global par défaut : `~/.cyonima/config.toml`.
    fn global_path() -> PathBuf {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(".cyonima").join("config.toml")
    }

    /// Ouvre (ou crée) le config manager. Charge la config globale depuis
    /// disque si elle existe, sinon utilise les defaults.
    pub async fn open() -> Self {
        let global_path = Self::global_path();
        let config = Self::load_from_file(&global_path).await;
        Self {
            inner: Arc::new(RwLock::new(config)),
            global_path,
        }
    }

    /// Charge la config depuis un fichier TOML. Silencieux : en cas d'erreur
    /// de parsing ou d'IO, renvoie les defaults.
    async fn load_from_file(path: &Path) -> Config {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }

    /// Lit la config courante (globale uniquement — le merge per-workspace
    /// se fait côté IPC ou dans le session manager).
    pub async fn get(&self) -> Config {
        self.inner.read().await.clone()
    }

    /// Met à jour le provider par défaut.
    pub async fn set_default_provider(&self, provider: Option<String>) -> anyhow::Result<()> {
        {
            let mut guard = self.inner.write().await;
            guard.provider.default_provider = provider;
        }
        self.flush().await
    }

    /// Met à jour le modèle par défaut.
    pub async fn set_default_model(&self, model: Option<String>) -> anyhow::Result<()> {
        {
            let mut guard = self.inner.write().await;
            guard.provider.default_model = model;
        }
        self.flush().await
    }

    /// Met à jour l'endpoint Ollama.
    pub async fn set_ollama_endpoint(&self, endpoint: Option<String>) -> anyhow::Result<()> {
        {
            let mut guard = self.inner.write().await;
            guard.provider.ollama_endpoint = endpoint;
        }
        self.flush().await
    }

    /// Met à jour un override de permission pour un outil.
    pub async fn set_permission_override(
        &self,
        tool: String,
        policy: String,
    ) -> anyhow::Result<()> {
        {
            let mut guard = self.inner.write().await;
            guard.permissions.overrides.insert(tool, policy);
        }
        self.flush().await
    }

    /// Supprime un override de permission.
    pub async fn remove_permission_override(&self, tool: &str) -> anyhow::Result<()> {
        {
            let mut guard = self.inner.write().await;
            guard.permissions.overrides.remove(tool);
        }
        self.flush().await
    }

    /// Charge une config per-workspace (`.cyonima/config.toml` dans le workspace).
    /// Renvoie None si le fichier n'existe pas.
    pub async fn load_workspace_config(workspace: &str) -> Option<Config> {
        let path = Path::new(workspace).join(".cyonima").join("config.toml");
        let config = Self::load_from_file(&path).await;
        // Si le fichier n'existait pas, load_from_file renvoie Default.
        // On distingue en vérifiant l'existence.
        if path.exists() {
            Some(config)
        } else {
            None
        }
    }

    /// Merge une config workspace sur la config globale. Les champs `Some` de
    /// workspace écrasent les champs de global.
    pub fn merge(global: &Config, workspace: &Config) -> Config {
        Config {
            storage: StorageConfig {
                models_dir: if workspace.storage.models_dir != StorageConfig::default().models_dir {
                    workspace.storage.models_dir.clone()
                } else {
                    global.storage.models_dir.clone()
                },
            },
            permissions: PermissionsConfig {
                overrides: {
                    let mut m = global.permissions.overrides.clone();
                    m.extend(workspace.permissions.overrides.clone());
                    m
                },
            },
            provider: ProviderConfig {
                default_provider: workspace
                    .provider
                    .default_provider
                    .clone()
                    .or_else(|| global.provider.default_provider.clone()),
                default_model: workspace
                    .provider
                    .default_model
                    .clone()
                    .or_else(|| global.provider.default_model.clone()),
                ollama_endpoint: workspace
                    .provider
                    .ollama_endpoint
                    .clone()
                    .or_else(|| global.provider.ollama_endpoint.clone()),
            },
        }
    }

    /// Écrit la config globale sur disque (atomic : temp + rename).
    async fn flush(&self) -> anyhow::Result<()> {
        let guard = self.inner.read().await;
        let toml_str = toml::to_string_pretty(&*guard)?;
        if let Some(parent) = self.global_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let tmp = self.global_path.with_extension("toml.tmp");
        tokio::fs::write(&tmp, &toml_str).await?;
        tokio::fs::rename(&tmp, &self.global_path).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_creates_default_config() {
        let tmp = std::env::temp_dir().join(format!(
            "cyonima-config-{}.toml",
            uuid::Uuid::new_v4()
        ));
        let cm = ConfigManager {
            inner: Arc::new(RwLock::new(Config::default())),
            global_path: tmp.clone(),
        };
        let cfg = cm.get().await;
        assert!(cfg.storage.models_dir.to_string_lossy().contains(".cyonima"));
        assert!(cfg.provider.default_provider.is_none());
        tokio::fs::remove_file(&tmp).await.ok();
    }

    #[tokio::test]
    async fn set_and_get_provider() {
        let tmp = std::env::temp_dir().join(format!(
            "cyonima-config-{}.toml",
            uuid::Uuid::new_v4()
        ));
        let cm = ConfigManager {
            inner: Arc::new(RwLock::new(Config::default())),
            global_path: tmp.clone(),
        };
        cm.set_default_provider(Some("openai".into())).await.unwrap();
        let cfg = cm.get().await;
        assert_eq!(cfg.provider.default_provider.as_deref(), Some("openai"));
        tokio::fs::remove_file(&tmp).await.ok();
    }

    #[tokio::test]
    async fn merge_prefers_workspace() {
        let mut global = Config::default();
        global.provider.default_provider = Some("ollama".into());
        global
            .permissions
            .overrides
            .insert("bash".into(), "deny".into());

        let mut workspace = Config::default();
        workspace.provider.default_provider = Some("openai".into());
        workspace
            .permissions
            .overrides
            .insert("bash".into(), "auto".into());
        workspace
            .permissions
            .overrides
            .insert("write_file".into(), "ask".into());

        let merged = ConfigManager::merge(&global, &workspace);
        assert_eq!(merged.provider.default_provider.as_deref(), Some("openai"));
        assert_eq!(
            merged.permissions.overrides.get("bash").map(|s| s.as_str()),
            Some("auto")
        );
        assert_eq!(
            merged.permissions.overrides.get("write_file").map(|s| s.as_str()),
            Some("ask")
        );
    }
}
