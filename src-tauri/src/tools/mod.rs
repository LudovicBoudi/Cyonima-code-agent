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
}

impl ToolOutput {
    pub fn ok(tool: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            output: output.into(),
            is_error: false,
        }
    }

    pub fn err(tool: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            output: msg.into(),
            is_error: true,
        }
    }
}

/// Abstraction universelle d'un outil agent. La factory [`ToolRegistry`]
/// route le tool-call LLM vers le bon [`Tool::execute`].
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
///sandboxé**. Toute tentative de remonter hors du workspace via `..` est
/// refusée. Renvoie le chemin absolu canonisé si OK, sinon une chaîne
/// d'erreur prête à_logger.
pub fn sandbox_resolve(workspace: &Path, target: &str) -> Result<PathBuf, String> {
    let target_path = Path::new(target);
    let is_absolute = target_path.is_absolute();
    let candidate = if is_absolute {
        PathBuf::from(target)
    } else {
        workspace.join(target)
    };
    // Canonisation : résout les `.` et `..`. Le workspace lui-même est
    // pré-canisé par le session manager (passé absolu).
    let canonical_workspace = workspace
        .canonicalize()
        .map_err(|e| format!("workspace injoignable: {e}"))?;
    let canonical_candidate = match candidate.canonicalize() {
        Ok(p) => p,
        // Fichier pas encore créé (cas write_file) : on canonicalise le parent
        // puis on réattache le nom.
        Err(_) => {
            let parent = candidate.parent().unwrap_or_else(|| Path::new(""));
            let canonical_parent = parent.canonicalize();
            let Ok(canon_parent) = canonical_parent else {
                return Err(format!(
                    "chemin invalide (parent injoignable): {}",
                    candidate.display()
                ));
            };
            let file_name = candidate
                .file_name()
                .ok_or_else(|| "nom de fichier vide".to_string())?;
            canon_parent.join(file_name)
        }
    };
    if !canonical_candidate.starts_with(&canonical_workspace) {
        return Err(format!(
            "chemin hors sandbox (workspace={} target={})",
            canonical_workspace.display(),
            canonical_candidate.display(),
        ));
    }
    Ok(canonical_candidate)
}

/// Tronque un contenu trop long pour le LLM (limite par défaut ~8 Ko).
/// Garde les N premières lignes + un marqueur `… (tronqué)`.
pub fn truncate_for_llm(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let truncated = s.split_at(max_bytes).0;
    format!("{truncated}\n… (tronqué, {}/{} octets)", max_bytes, s.len())
}
