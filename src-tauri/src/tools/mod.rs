//! Outils agent : lecture/écriture de fichiers, recherche, exécution shell.
//!
//! Tous les outils passent par le gateway [`crate::permissions`]. Pas de
//! bypass : un outil ne s'exécute jamais sans que `permissions::Gateway` ait
//! autorisé, et l'autorisation est tracée via un `Decision::Allow` ou
//! `Decision::Deny` renvoyé par l'utilisateur (ou auto selon la `Policy`).
//!
//! Le sandboxing est workspace-rooted : tous les chemins passés à un outil
//! sont résolus relatifs au `<workspace>` configuré pour la session. Toute
//! tentative de remonter via `..` hors du workspace renvoie une erreur.
//!
//! Implémentations concrètes (J3) :
//! - [`read_file`]   : lecture fichier
//! - [`write_file`]  : écriture fichier (création/écrasement)
//! - [`edit_file`]   : remplacement de texte exact dans un fichier
//! - [`glob`]        : recherche de fichiers par pattern
//! - [`grep`]        : recherche de contenu par regex
//! - [`bash`]        : exécution de commande shell (preview + approbation)

pub mod bash;
pub mod edit_file;
pub mod glob;
pub mod grep;
pub mod read_file;
pub mod semantic_search;
pub mod write_file;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Spécification exportée vers le provider pour le tool-use (API Ollama / OpenAI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// Schéma JSON des arguments attendus (format OpenAI function calling).
    pub parameters: serde_json::Value,
}

/// Résultat d'exécution d'un outil, sérializé et renvoyé au LLM comme
/// message `tool` dans le contexte.
#[derive(Debug, Clone, Serialize)]
pub struct ToolOutput {
    pub tool: String,
    /// Texte montré au LLM (résumé, contenu, message d'erreur…).
    pub output: String,
    /// `true` si l'exécution a échoué côté filesystem / shell / parsing args.
    /// Permet au LLM de comprendre qu'il doit corriger son appel.
    pub is_error: bool,
    /// Texte compact destiné au LLM quand `output` contient un payload riche
    /// (ex. diff complet pour l'UI). `None` = on utilise `output`.
    #[serde(skip)]
    pub llm_text: Option<String>,
}

impl ToolOutput {
    pub fn ok(tool: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            output: output.into(),
            is_error: false,
            llm_text: None,
        }
    }

    pub fn err(tool: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            output: msg.into(),
            is_error: true,
            llm_text: None,
        }
    }

    /// Fournit un texte compact à envoyer au LLM à la place de `output`
    /// (utile quand `output` embarque un payload lourd pour l'UI).
    pub fn with_llm_text(mut self, text: impl Into<String>) -> Self {
        self.llm_text = Some(text.into());
        self
    }

    /// Texte à réinjecter dans le contexte LLM.
    pub fn for_llm(&self) -> &str {
        self.llm_text.as_deref().unwrap_or(self.output.as_str())
    }
}

/// Abstraction universelle d'un outil agent. La factory [`ToolRegistry`]
/// route le tool-call LLM vers le bon [`Tool::execute`].
// `async_trait` ajoute `#[must_use]` au future boxé, qui est déjà `#[must_use]`
// (le trait `Future` l'est), d'où le lint `double_must_use` (Clippy ≥ 1.99).
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn spec(&self) -> ToolSpec;
    /// Exécute. `args` est la JSON envoyée par le LLM (valisée au mieux).
    /// `workspace` est la racine sandbox.
    async fn execute(&self, args: serde_json::Value, workspace: &Path) -> ToolOutput;
}

/// Registry des outils activés pour une session.
#[derive(Clone)]
pub struct ToolRegistry {
    tools: Arc<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    /// Construit le registry avec l'ensemble des outils built-in activés.
    /// On active tous les outils standards par défaut ; la config par projet
    /// viendra plus tard pour désactiver/griser certains.
    pub fn built_in() -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let entries: Vec<Arc<dyn Tool>> = vec![
            Arc::new(read_file::ReadFile),
            Arc::new(write_file::WriteFile),
            Arc::new(edit_file::EditFile),
            Arc::new(glob::Glob),
            Arc::new(grep::Grep),
            Arc::new(bash::Bash::new()),
            Arc::new(semantic_search::SemanticSearch),
        ];
        for t in entries {
            tools.insert(t.name().to_string(), t);
        }
        Self {
            tools: Arc::new(tools),
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|t| t.spec()).collect()
    }

    pub async fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
        workspace: &Path,
    ) -> Option<ToolOutput> {
        let t = self.tools.get(name)?;
        Some(t.execute(args, workspace).await)
    }
}

/// Résout un chemin relatif au workspace en chemin absolu **strictement
/// sandboxé**. Toute tentative de sortir du workspace (via `..`, chemin absolu
/// hors projet, ou symlink pointant à l'extérieur) est refusée.
///
/// Toutes les branches vérifient `starts_with(canonical_workspace)` sur un
/// chemin **canonisé** (symlinks résolus). Pour un fichier inexistant (création),
/// on canonicalise le plus proche ancêtre existant puis on reconstruit le
/// chemin — ce qui autorise la création de sous-dossiers sans ouvrir de brèche.
pub fn sandbox_resolve(workspace: &Path, target: &str) -> Result<PathBuf, String> {
    let canonical_workspace = workspace
        .canonicalize()
        .map_err(|e| format!("workspace injoignable: {e}"))?;

    let target_path = Path::new(target);

    // On refuse tout composant `..` : plus simple et sûr que de le résoudre
    // (un chemin non existant ne peut pas être canonicalisé de façon fiable).
    if target_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!(
            "chemin refusé: `{target}` contient `..` (hors workspace)"
        ));
    }

    let candidate = if target_path.is_absolute() {
        target_path.to_path_buf()
    } else {
        canonical_workspace.join(target_path)
    };

    // Cas 1 : le chemin existe → canonicalise (résout les symlinks) et vérifie.
    if let Ok(canonical) = candidate.canonicalize() {
        if canonical.starts_with(&canonical_workspace) {
            return Ok(canonical);
        }
        return Err(format!(
            "chemin hors workspace: `{target}` (résolu en {})",
            canonical.display()
        ));
    }

    // Cas 2 : le chemin n'existe pas encore (création). On remonte jusqu'au
    // plus proche ancêtre existant, on vérifie qu'il est dans le workspace,
    // puis on ré-ajoute les composants restants (sans `..`, déjà rejeté).
    let mut ancestor = candidate.as_path();
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(canonical_ancestor) = ancestor.canonicalize() {
            if !canonical_ancestor.starts_with(&canonical_workspace) {
                return Err(format!(
                    "chemin hors workspace: `{target}` (ancêtre {} hors projet)",
                    canonical_ancestor.display()
                ));
            }
            let mut resolved = canonical_ancestor;
            for part in suffix.iter().rev() {
                resolved.push(part);
            }
            return Ok(resolved);
        }
        let Some(name) = ancestor.file_name() else {
            return Err(format!("chemin invalide: `{target}`"));
        };
        suffix.push(name.to_os_string());
        let Some(parent) = ancestor.parent() else {
            return Err(format!("chemin invalide: `{target}`"));
        };
        ancestor = parent;
    }
}

/// Tronque un contenu trop long pour le LLM (limite par défaut ~8 Ko).
/// Garde les N premiers octets **sur une frontière de caractère** (pas de panic
/// sur contenu UTF-8 multioctet) + un marqueur `… (tronqué)`.
pub fn truncate_for_llm(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… (tronqué, {end}/{} octets)", &s[..end], s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_ws() -> PathBuf {
        let p = std::env::temp_dir().join(format!("cyonima-sandbox-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn sandbox_rejects_parent_dir() {
        let ws = tmp_ws();
        assert!(sandbox_resolve(&ws, "../etc/passwd").is_err());
        assert!(sandbox_resolve(&ws, "a/../../etc/passwd").is_err());
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn sandbox_rejects_absolute_outside() {
        let ws = tmp_ws();
        let outside = if cfg!(windows) {
            "C:\\Windows\\win.ini"
        } else {
            "/etc/passwd"
        };
        assert!(sandbox_resolve(&ws, outside).is_err());
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn sandbox_allows_relative_inside_nonexistent() {
        let ws = tmp_ws();
        let resolved = sandbox_resolve(&ws, "sub/dir/new_file.rs").unwrap();
        assert!(resolved.starts_with(ws.canonicalize().unwrap()));
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn sandbox_allows_existing_inside() {
        let ws = tmp_ws();
        std::fs::write(ws.join("f.txt"), "x").unwrap();
        let resolved = sandbox_resolve(&ws, "f.txt").unwrap();
        assert!(resolved.ends_with("f.txt"));
        assert!(resolved.starts_with(ws.canonicalize().unwrap()));
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn truncate_is_char_boundary_safe() {
        // "é" = 2 octets : couper à l'octet 1 ne doit pas paniquer.
        let s = "ééééé";
        let out = truncate_for_llm(s, 1);
        assert!(out.contains("tronqué"));
        // Ne doit pas paniquer non plus à chaque frontière.
        for n in 0..s.len() + 1 {
            let _ = truncate_for_llm(s, n);
        }
    }
}
