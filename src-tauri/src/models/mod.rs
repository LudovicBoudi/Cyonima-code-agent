//! Registry, catalogue et downloader de modèles.
//!
//! MIGRATION OLLAMA : Plus de téléchargement GGUF direct. 
//! Seuls le registry et le catalogue Ollama restent actifs.
//! - **Modèles Ollama** : gérés via `ollama pull`
//! - **Modèles custom** : import via [`import_custom`] 

// pub mod downloader; // OBSOLÈTE - utiliser Ollama
pub mod registry;

use serde::{Deserialize, Serialize};

/// Catalogue embarqué (compilé dans le binaire via include_str!).
/// Position relative au crate `src-tauri` (la racine Cargo).
const CATALOG_TOML: &str = include_str!("../../../docs/models-catalog.toml");

/// Modèle installé ou installable, exposé à l'UI via IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub quantization: String,
    /// Licence courte affichée avant téléchargement.
    pub license: String,
    /// RAM minimum recommandée en Go (CPU pur). Garde-fou côté backend.
    pub ram_min_gb: u32,
    /// Type de modèle : "general" pour généraliste, "coding" pour orienté code.
    pub model_type: String,
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

/// Entrée du catalogue Ollama (cf `docs/models-catalog.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub quantization: String,
    pub license: String,
    pub ram_min_gb: u32,
    /// Type de modèle : "general" pour généraliste, "coding" pour orienté code.
    #[serde(default = "default_model_type")]
    pub model_type: String,
    /// Tag Ollama pour pull direct (OBLIGATOIRE).
    pub ollama_tag: String,
}

fn default_model_type() -> String {
    "general".to_string()
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

/// Renvoie les entrées brutes du catalogue (pour compatibilité legacy). 
/// OBSOLÈTE : plus utilisé pour les téléchargements directs.
pub fn find_catalog_entry(id: &str) -> Option<CatalogEntry> {
    parse_catalog().into_iter().find(|e| e.id == id)
}

/// Renvoie le catalogue avec statut `installed` calculé à partir du registry et Ollama.
pub async fn list_catalog(registry: &registry::Registry) -> Vec<ModelInfo> {
    let installed = registry.list().await;
    
    // Récupère les modèles déjà installés dans Ollama (localhost:11434).
    // Si Ollama n'est pas joignable, on continue avec une liste vide plutôt
    // que d'échouer : le catalogue reste consultable hors-ligne.
    let ollama_models = crate::ipc::fetch_ollama_models("http://localhost:11434")
        .await
        .unwrap_or_default();
    let ollama_tags: std::collections::HashSet<String> = ollama_models
        .iter()
        .map(|m| m.name.clone())
        .collect();

    parse_catalog()
        .into_iter()
        .map(|e| {
            let inst = installed.iter().find(|i| i.id == e.id);
            let ollama_installed = ollama_tags.contains(&e.ollama_tag);
            
            ModelInfo {
                id: e.id.clone(),
                name: e.name.clone(),
                size_bytes: 0, // Pas pertinent pour Ollama
                quantization: e.quantization.clone(),
                license: e.license.clone(),
                ram_min_gb: e.ram_min_gb,
                model_type: e.model_type.clone(),
                installed: inst.is_some() || ollama_installed,
                ollama_tag: Some(e.ollama_tag.clone()),
                url: None, // Plus d'URL directe
                installed_path: inst.map(|i| i.path.clone()).or_else(|| {
                    // Pour les modèles Ollama, indiquer qu'ils sont "installés via Ollama"
                    if ollama_installed {
                        Some("ollama".to_string())
                    } else {
                        None
                    }
                }),
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
            model_type: e.model_type.unwrap_or_else(|| "general".to_string()),
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
        model_type: Some("general".to_string()),
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
            assert!(!m.ollama_tag.is_empty(), "Gemma 4 doit avoir un tag Ollama");
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
