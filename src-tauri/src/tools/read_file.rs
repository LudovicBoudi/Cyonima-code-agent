//! Outil `read_file` — lecture d'un fichier dans le workspace sandboxé.

use std::path::Path;

use async_trait::async_trait;

use super::{sandbox_resolve, truncate_for_llm, Tool, ToolOutput, ToolSpec};

pub struct ReadFile;

#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".into(),
            description: "Lit le contenu d'un fichier du workspace. Le chemin est relatif au workspace racine.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Chemin relatif au workspace" }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value, workspace: &Path) -> ToolOutput {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return ToolOutput::err("read_file", "argument `path` (string) manquant");
        };
        match sandbox_resolve(workspace, path) {
            Ok(abs) => match tokio::fs::read_to_string(&abs).await {
                Ok(content) => ToolOutput::ok("read_file", truncate_for_llm(&content, 8_192)),
                Err(e) => {
                    ToolOutput::err("read_file", format!("échec lecture {}: {e}", abs.display()))
                }
            },
            Err(e) => ToolOutput::err("read_file", e),
        }
    }
}
