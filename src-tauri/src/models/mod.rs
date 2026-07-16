//! Registry, catalogue et downloader de modèles.
//!
//! Cf cahier des charges :
//! - **< 50 Mo** : embarqués en ressources (`src-tauri/resources/`). Actuellement
//!   un embedder `all-MiniLM-L6-v2` Q8 pour la recherche sémantique zero-config.
//! - **> 50 Mo open source** : listés dans le catalogue TOML embarqué
//!   (`docs/models-catalog.toml`), parsé une fois au démarrage via `include_str!`.
//!   Ils sont téléchargeables via le downloader (J4) ou, si un `ollama_tag` est
//!   renseigné, utilisables directement via le provider Ollama sans download
//!   propre à Cyonima (l'utilisateur fait `ollama pull <tag>`).
//! - **Entreprise / custom** : [`import_custom`] enregistre un GGUF local
//!   dans le registry.
//!
//! Le downloader concret (HTTP range + SHA256 + reprise) arrive en J4. La
//! lecture du catalogue, elle, est fonctionnelle dès J1.5 et exposée via
//! `model_catalog_list` IPC.

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
    pub installed: bool,
    /// Tag Ollama correspondant, si applicable (ex: "gemma4:12b").
    /// Quand renseigné, l'UI permet d'utiliser le modèle directement via
    /// Ollama sans téléchargement Cyonima.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ollama_tag: Option<String>,
    /// URL de téléchargement du GGUF HuggingFace, si applicable.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub url: Option<String>,
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

/// Renvoie les modèles installés localement. Vraie implémentation en J4 —
/// pour l'instant on lit le dossier `models` configurable et on signale
/// ceux dont on trouve un `.gguf`. J1.5 : retour vide (le downloader n'existe
/// pas encore), mais l'UI peut afficher le catalogue dès maintenant.
pub fn list_installed() -> Vec<ModelInfo> {
    Vec::new()
}

/// Renvoie le catalogue complet embarqué dans le binaire. Appelé par l'IPC
/// `model_catalog_list`. Aucun accès filesystem : le TOML est `include_str!`.
pub fn list_catalog() -> Vec<ModelInfo> {
    parse_catalog()
        .into_iter()
        .map(|e| ModelInfo {
            id: e.id,
            name: e.name,
            size_bytes: e.size_bytes,
            quantization: e.quantization,
            license: e.license,
            ram_min_gb: e.ram_min_gb,
            installed: false,
            ollama_tag: e.ollama_tag,
            url: Some(e.url).filter(|u| !u.is_empty()),
        })
        .collect()
}

/// Placeholder J5 — import d'un GGUF entreprise. Vraie implémentation en J5.
pub fn import_custom(path: &str) -> anyhow::Result<ModelInfo> {
    // Vérifie juste que le chemin pointe vers un fichier .gguf existant.
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
    Ok(ModelInfo {
        id: format!("custom/{name}"),
        name,
        size_bytes,
        quantization: "unknown".into(),
        license: "Unspecified".into(),
        ram_min_gb: 0,
        installed: true,
        ollama_tag: None,
        url: Some(path.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_parses_and_has_entries() {
        let catalog = list_catalog();
        assert!(!catalog.is_empty(), "le catalogue ne doit pas être vide");
        // Chaque entrée doit avoir un id unique.
        let mut ids: Vec<_> = catalog.iter().map(|m| m.id.clone()).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total, "IDs en double dans le catalogue");
    }

    #[test]
    fn gemma4_entries_have_apache_license() {
        let catalog = list_catalog();
        let g4: Vec<_> = catalog.iter().filter(|m| m.id.starts_with("gemma-4-")).collect();
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
    fn import_custom_rejects_missing() {
        let r = import_custom("nonexistent.gguf");
        assert!(r.is_err());
    }
}