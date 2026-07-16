//! Registry, catalogue et downloader de modèles.
//!
//! Cf cahier des charges :
//! - **< 50 Mo** : embarqués en ressources (`src-tauri/resources/`). Actuellement
//!   un embedder `all-MiniLM-L6-v2` Q8 pour la recherche sémantique zero-config.
//! - **> 50 Mo open source** : listés dans [`CatalogEntry`] (TOML), téléchargeables
//!   via [`Downloader`] (HTTP range + SHA256 + reprise).
//! - **Entreprise / custom** : [`import_custom`] enregistre un GGUF local dans le
//!   registry.
//!
//! Le downloader concret est câblé en J4. Ce module expose les types pour que
//! le squelette IPC compile.

use serde::{Deserialize, Serialize};

/// Modèle installé ou installable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    /// Taille en octets (0 si inconnue pour un custom).
    pub size_bytes: u64,
    pub quantization: String,
    /// Licence courte affichée avant téléchargement.
    pub license: String,
    /// RAM minimum recommandée en Go.
    pub ram_min_gb: u32,
    pub installed: bool,
}

/// Entrée du catalogue distant téléchargeable (cf `docs/models-catalog.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    /// URL de téléchargement (HuggingFace, Ollama Library ou miroir).
    pub url: String,
    /// SHA256 attendu (vérifié post-download).
    pub sha256: String,
    pub size_bytes: u64,
    pub quantization: String,
    pub license: String,
    pub ram_min_gb: u32,
}

/// Placeholder J0 — l'implémentation concrète arrive en J4.
pub fn list_installed() -> Vec<ModelInfo> {
    Vec::new()
}

pub fn list_catalog() -> Vec<ModelInfo> {
    Vec::new()
}

/// Placeholder J5 — import d'un GGUF entreprise.
pub fn import_custom(_path: &str) -> anyhow::Result<ModelInfo> {
    Ok(ModelInfo {
        id: "custom".into(),
        name: "Custom model".into(),
        size_bytes: 0,
        quantization: "unknown".into(),
        license: "Unspecified".into(),
        ram_min_gb: 0,
        installed: false,
    })
}
