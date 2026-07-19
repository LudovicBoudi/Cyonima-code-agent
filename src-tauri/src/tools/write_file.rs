//! Outil `write_file` — écriture/écrasement d'un fichier du workspace.
//!
//! Passe par la permission `Policy::Ask` (cf `permissions::default_policy`).
//! Le gateway est appelé en amont par le `SessionManager`, qui ne déclenche
//! l'exécution de l'outil qu'après `Decision::Allow`. Cet outil ne fait donc
//! pas de contrôle autorisation lui-même.

use std::path::Path;

use async_trait::async_trait;

use super::{sandbox_resolve, Tool, ToolOutput, ToolSpec};

pub struct WriteFile;

#[async_trait]
impl Tool for WriteFile {
    fn name(&self) -> &str {
        "write_file"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file".into(),
            description: "Écrit (ou écrase) un fichier dans le workspace.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Chemin relatif au workspace" },
                    "content": { "type": "string", "description": "Contenu intégral à écrire" }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value, workspace: &Path) -> ToolOutput {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return ToolOutput::err("write_file", "argument `path` (string) manquant");
        };
        let Some(content) = args.get("content").and_then(|v| v.as_str()) else {
            return ToolOutput::err("write_file", "argument `content` (string) manquant");
        };
        match sandbox_resolve(workspace, path) {
            Ok(abs) => {
                // Crée le dossier parent si nécessaire — surgery workspace only.
                if let Some(parent) = abs.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        return ToolOutput::err(
                            "write_file",
                            format!("échec mkdir {}: {e}", parent.display()),
                        );
                    }
                }
                match tokio::fs::write(&abs, content).await {
                    Ok(_) => ToolOutput::ok(
                        "write_file",
                        format!("écrit {} ({} octets)", abs.display(), content.len()),
                    ),
                    Err(e) => ToolOutput::err(
                        "write_file",
                        format!("échec écriture {}: {e}", abs.display()),
                    ),
                }
            }
            Err(e) => ToolOutput::err("write_file", e),
        }
    }
}
