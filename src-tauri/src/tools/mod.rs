//! Outils agent : lecture/écriture de fichiers, recherche, exécution.
//!
//! Tous les outils passent par le gateway [`crate::permissions`]. Pas de
//! bypass : un outil ne s'exécute jamais sans que `permissions::authorize`
//! ait été appelé.
//!
//! Implémentation concrète prévue au jalon J3. Squelette J0 uniquement.

use serde::{Deserialize, Serialize};

/// Signature d'un outil, exposée au provider pour le tool-use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// Schéma JSON des arguments attendus.
    pub parameters: serde_json::Value,
}

/// Résultat d'exécution d'un outil.
#[derive(Debug, Clone, Serialize)]
pub struct ToolOutput {
    pub tool: String,
    pub output: String,
    /// `true` si l'exécution a échoué côté filesystem / shell.
    pub is_error: bool,
}

/// Catalogue des outils intégrés en v1.
pub fn builtin_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "read_file".into(),
            description: "Lit le contenu d'un fichier du workspace.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "glob".into(),
            description: " Cherche des fichiers par pattern (ex: **/*.rs).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "pattern": { "type": "string" } },
                "required": ["pattern"]
            }),
        },
    ]
}
