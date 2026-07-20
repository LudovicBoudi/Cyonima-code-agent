//! Registry des modèles installés — persistance JSON dans
//! `~/.cyonima/models/registry.json`.
//!
//! Le registry distingue :
//! - modèles téléchargés via Cyonima (entry avec path + sha256)
//! - modèles importés custom (entry avec path pointant vers le GGUF choisi
//!   par l'utilisateur, sans sha256 calculé par défaut)
//!
//! Les chemins sont stockés absolus pour éviter toute ambiguïté quand
//! l'utilisateur change de workspace. L'emplacement du registry lui-même
//! est configurable via `~/.cyonima/config.toml` (J9).

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    /// Chemin absolu du .gguf sur disque.
    pub path: String,
    pub size_bytes: u64,
    /// SHA256 calculé à la fin du download (vide pour les imports custom).
    pub sha256: String,
    pub quantization: String,
    pub license: String,
    pub ram_min_gb: u32,
    /// Type de modèle : "general" pour généraliste, "coding" pour orienté code.
    #[serde(default)]
    pub model_type: Option<String>,
    /// Tag Ollama optionnel (déclaré par le catalogue TomL).
    pub ollama_tag: Option<String>,
    /// URL source (vide pour imports custom).
    pub url: String,
    pub downloaded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryFile {
    pub entries: Vec<RegistryEntry>,
}

/// Registry partagé via `AppState`, chargé lazy à la première lecture et
/// flushé après chaque mutation. Le `RwLock` permet la lecture concurrente
/// (UI catalogue, IPC list) et l'écriture exclusive (post-download).
#[derive(Clone)]
pub struct Registry {
    inner: Arc<RwLock<RegistryFile>>,
    path: Arc<PathBuf>,
}

impl Registry {
    /// Ouvre (ou crée vide) le registry au chemin donné.
    pub async fn open(path: PathBuf) -> anyhow::Result<Self> {
        let entries = if let Ok(content) = tokio::fs::read_to_string(&path).await {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            RegistryFile::default()
        };
        Ok(Self {
            inner: Arc::new(RwLock::new(entries)),
            path: Arc::new(path),
        })
    }

    /// Ouvre le registry à l'emplacement par défaut `<cyonima_dir>/models/registry.json`
    /// en créant le dossier parent si nécessaire. `cyonima_dir` = `~/.cyonima`.
    pub async fn default_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".cyonima").join("models").join("registry.json")
    }

    pub async fn open_default() -> anyhow::Result<Self> {
        let path = Self::default_path().await;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        Self::open(path).await
    }

    /// Liste les modèles installés.
    pub async fn list(&self) -> Vec<RegistryEntry> {
        self.inner.read().await.entries.clone()
    }

    /// Renvoie `true` si un modèle avec cet ID est enregistré.
    pub async fn contains(&self, model_id: &str) -> bool {
        self.inner
            .read()
            .await
            .entries
            .iter()
            .any(|e| e.id == model_id)
    }

    /// Insère ou remplace une entrée par ID. Flush immédiat sur disque.
    pub async fn upsert(&self, entry: RegistryEntry) -> anyhow::Result<()> {
        {
            let mut guard = self.inner.write().await;
            guard.entries.retain(|e| e.id != entry.id);
            guard.entries.push(entry);
        }
        self.flush().await
    }

    /// Supprime une entrée du registry (sans supprimer le fichier, c'est à
    /// l'appelant de le faire — par exemple l'UI "Supprimer le modèle").
    pub async fn remove(&self, model_id: &str) -> anyhow::Result<()> {
        {
            let mut guard = self.inner.write().await;
            guard.entries.retain(|e| e.id != model_id);
        }
        self.flush().await
    }

    /// Renvoie le chemin du GGUF d'un modèle installé, s'il existe encore sur disque.
    pub async fn path_of(&self, model_id: &str) -> Option<PathBuf> {
        let guard = self.inner.read().await;
        let entry = guard.entries.iter().find(|e| e.id == model_id)?;
        let p = PathBuf::from(&entry.path);
        if p.exists() {
            Some(p)
        } else {
            None
        }
    }

    async fn flush(&self) -> anyhow::Result<()> {
        let guard = self.inner.read().await;
        let json = serde_json::to_string_pretty(&*guard)?;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        // Écriture atomique : temp puis rename, pour éviter de corrompre le
        // registry si l'app crash en plein milieu.
        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, json).await?;
        tokio::fs::rename(&tmp, self.path.as_ref()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upsert_then_list_then_remove() {
        let tmp =
            std::env::temp_dir().join(format!("cyonima-registry-{}.json", uuid::Uuid::new_v4()));
        let r = Registry::open(tmp.clone()).await.unwrap();
        assert!(r.list().await.is_empty());

        let entry = RegistryEntry {
            id: "test".into(),
            name: "Test".into(),
            path: "/tmp/x.gguf".into(),
            size_bytes: 42,
            sha256: "abc".into(),
            quantization: "Q4".into(),
            license: "MIT".into(),
            ram_min_gb: 4,
            model_type: Some("general".to_string()),
            ollama_tag: None,
            url: "".into(),
            downloaded_at: Utc::now(),
        };
        r.upsert(entry.clone()).await.unwrap();
        assert_eq!(r.list().await.len(), 1);
        assert!(r.contains("test").await);

        // Replace (même id).
        let mut entry2 = entry.clone();
        entry2.size_bytes = 100;
        r.upsert(entry2).await.unwrap();
        assert_eq!(r.list().await.len(), 1);
        assert_eq!(r.list().await[0].size_bytes, 100);

        r.remove("test").await.unwrap();
        assert!(r.list().await.is_empty());
        tokio::fs::remove_file(&tmp).await.ok();
    }
}
