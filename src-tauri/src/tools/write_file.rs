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
            description: "Écrit un fichier dans le workspace. Le chemin doit rester dans le workspace (relatif ou absolu). Les chemins hors workspace sont rejetés.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Chemin du fichier (relatif ex: src/main.rs, ou absolu dans le workspace)" },
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
                // Lire le contenu existant avant écrasement (pour le diff).
                let before = tokio::fs::read_to_string(&abs).await.unwrap_or_default();
                let created = before.is_empty();

                if let Some(parent) = abs.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        return ToolOutput::err(
                            "write_file",
                            format!("échec mkdir {}: {e}", parent.display()),
                        );
                    }
                }
                match tokio::fs::write(&abs, content).await {
                    Ok(_) => {
                        let summary = if created {
                            format!("fichier créé ({} octets)", content.len())
                        } else {
                            format!("fichier écrasé ({} octets)", content.len())
                        };
                        // Sortie JSON structurée pour le diff viewer frontend.
                        let output = serde_json::json!({
                            "message": summary,
                            "diff_info": {
                                "path": path,
                                "before": before,
                                "after": content,
                                "created": created,
                            },
                        });
                        ToolOutput::ok("write_file", output.to_string())
                    }
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
