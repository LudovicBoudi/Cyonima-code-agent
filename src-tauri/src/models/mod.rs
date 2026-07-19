//! Registry, catalogue et downloader de modèles.
//!
//! Cf cahier des charges :
//! - **< 50 Mo** : embarqués en ressources (`src-tauri/resources/`).
//! - **> 50 Mo open source** : listés dans le catalogue TOML embarqué
//!   (`docs/models-catalog.toml`), parsé une fois au démarrage via
//!   `include_str!`. Téléchargeables via [`downloader::DownloadManager`].
//! - **Entreprise / custom** : [`import_custom`] enregistre un GGUF local
//!   dans le [`registry::Registry`].

pub mod downloader;
pub mod registry;

use serde::{Deserialize, Serialize};

/// Catalogue embarqué (compilé dans le binaire via include_str!).
/// Position relative au crate `src-tauri` (la racine Cargo).
const CATALOG_TOML: &str = include_str!("../../../docs/models-catalog.toml");

/// Modèle installé ou installable, exposé à l'UI via IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub quantization: String,
    /// Licence courte affichée avant téléchargement.
    pub license: String,
    /// RAM minimum recommandée en Go (CPU pur). Garde-fou côté backend.
    pub ram_min_gb: u32,
    /// `true` si enregistré dans le registry ET fichier encore présent sur disque.
    pub installed: bool,
    /// Tag Ollama correspondant, si applicable (ex: "gemma4:12b").
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ollama_tag: Option<String>,
    /// URL de téléchargement du GGUF HuggingFace, si applicable.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub url: Option<String>,
    /// Chemin absolu du .gguf si installé.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub installed_path: Option<String>,
}

/// Entrée du catalogue distant téléchargeable (cf `docs/models-catalog.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub quantization: String,
    pub license: String,
    pub ram_min_gb: u32,
    /// Champ optionnel : tag Ollama pour usage direct sans download Cyonima.
    pub ollama_tag: Option<String>,
}

/// Structure racine du TOML : N entrées `[[models]]`.
#[derive(Debug, Deserialize)]
struct CatalogFile {
    models: Vec<CatalogEntry>,
}

fn parse_catalog() -> Vec<CatalogEntry> {
    match toml::from_str::<CatalogFile>(CATALOG_TOML) {
        Ok(f) => f.models,
        Err(e) => {
            tracing::error!("Échec parsage du catalogue embarqué: {e}");
            Vec::new()
        }
    }
}

/// Renvoie les entrées brutes du catalogue (avec URL + SHA256). Utilisé par
/// le `DownloadManager` pour récupérer l'entry à télécharger.
pub fn find_catalog_entry(id: &str) -> Option<CatalogEntry> {
    parse_catalog().into_iter().find(|e| e.id == id)
}

/// Renvoie le catalogue avec statut `installed` calculé à partir du registry.
pub async fn list_catalog(registry: &registry::Registry) -> Vec<ModelInfo> {
    let installed = registry.list().await;
    parse_catalog()
        .into_iter()
        .map(|e| {
            let inst = installed.iter().find(|i| i.id == e.id);
            ModelInfo {
                id: e.id.clone(),
                name: e.name.clone(),
                size_bytes: e.size_bytes,
                quantization: e.quantization.clone(),
                license: e.license.clone(),
                ram_min_gb: e.ram_min_gb,
                installed: inst.is_some(),
                ollama_tag: e.ollama_tag.clone(),
                url: Some(e.url.clone()).filter(|u| !u.is_empty()),
                installed_path: inst.map(|i| i.path.clone()),
            }
        })
        .collect()
}

/// Renvoie les modèles installés (depuis le registry), augmentés d'une
/// vérification que le .gguf existe encore sur disque.
pub async fn list_installed(registry: &registry::Registry) -> Vec<ModelInfo> {
    registry
        .list()
        .await
        .into_iter()
        .filter(|e| std::path::Path::new(&e.path).exists())
        .map(|e| ModelInfo {
            id: e.id,
            name: e.name,
            size_bytes: e.size_bytes,
            quantization: e.quantization,
            license: e.license,
            ram_min_gb: e.ram_min_gb,
            installed: true,
            ollama_tag: e.ollama_tag,
            url: Some(e.url).filter(|u| !u.is_empty()),
            installed_path: Some(e.path),
        })
        .collect()
}

/// Import d'un GGUF entreprise/personnel. Vérifie seulement que le chemin
/// pointe vers un fichier .gguf lisible. L'enregistrement dans le registry
/// est fait par l'IPC `model_import_custom` qui appelle cette fonction puis
/// `registry.upsert`.
pub fn validate_custom(path: &str) -> anyhow::Result<registry::RegistryEntry> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        anyhow::bail!("Fichier introuvable: {path}");
    }
    let is_gguf = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false);
    if !is_gguf {
        anyhow::bail!("Le fichier doit avoir l'extension .gguf");
    }
    let name = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("custom")
        .to_string();
    let size_bytes = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    Ok(registry::RegistryEntry {
        id: format!("custom/{name}"),
        name,
        path: p.to_string_lossy().to_string(),
        size_bytes,
        sha256: String::new(),
        quantization: "unknown".into(),
        license: "Unspecified".into(),
        ram_min_gb: 0,
        ollama_tag: None,
        url: String::new(),
        downloaded_at: chrono::Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_parses_and_has_entries() {
        let catalog = parse_catalog();
        assert!(!catalog.is_empty(), "le catalogue ne doit pas être vide");
        let mut ids: Vec<_> = catalog.iter().map(|m| m.id.clone()).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total, "IDs en double dans le catalogue");
    }

    #[test]
    fn gemma4_entries_have_apache_license() {
        let catalog = parse_catalog();
        let g4: Vec<_> = catalog
            .iter()
            .filter(|m| m.id.starts_with("gemma-4-"))
            .collect();
        assert!(!g4.is_empty(), "Gemma 4 doit être présent");
        for m in &g4 {
            assert!(
                m.license.contains("Apache-2.0"),
                "Gemma 4 devrait être Apache-2.0, trouvé: {}",
                m.license
            );
            assert!(m.ollama_tag.is_some(), "Gemma 4 doit avoir un tag Ollama");
        }
    }

    #[test]
    fn find_catalog_entry_returns_some_for_known() {
        assert!(find_catalog_entry("gemma-4-12b-it-qat-q4_0").is_some());
        assert!(find_catalog_entry("inexistant-id").is_none());
    }

    #[test]
    fn validate_custom_rejects_missing() {
        let r = validate_custom("nonexistent.gguf");
        assert!(r.is_err());
    }
}
