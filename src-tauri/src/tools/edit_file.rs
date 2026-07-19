//! Outil `edit_file` — remplacement précis de texte dans un fichier.
//!
//! Politique prudente : `Ask` (cf `permissions::default_policy`). Le
//! `old_string` doit exister **exactement une fois** dans le fichier pour
//! éviter les éditions ambiguës.

use std::path::Path;

use async_trait::async_trait;

use super::{sandbox_resolve, Tool, ToolOutput, ToolSpec};

pub struct EditFile;

#[async_trait]
impl Tool for EditFile {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "edit_file".into(),
            description: "Remplace un fragment de texte exact dans un fichier. `old_string` doit apparaître exactement une fois.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old_string": { "type": "string", "description": "Texte à remplacer (doit être unique)" },
                    "new_string": { "type": "string", "description": "Remplaçant" }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value, workspace: &Path) -> ToolOutput {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return ToolOutput::err("edit_file", "argument `path` manquant");
        };
        let Some(old_str) = args.get("old_string").and_then(|v| v.as_str()) else {
            return ToolOutput::err("edit_file", "argument `old_string` manquant");
        };
        let Some(new_str) = args.get("new_string").and_then(|v| v.as_str()) else {
            return ToolOutput::err("edit_file", "argument `new_string` manquant");
        };
        let abs = match sandbox_resolve(workspace, path) {
            Ok(a) => a,
            Err(e) => return ToolOutput::err("edit_file", e),
        };
        let content = match tokio::fs::read_to_string(&abs).await {
            Ok(c) => c,
            Err(e) => {
                return ToolOutput::err(
                    "edit_file",
                    format!("échec lecture {}: {e}", abs.display()),
                )
            }
        };
        let count = content.matches(old_str).count();
        if count == 0 {
            return ToolOutput::err(
                "edit_file",
                format!("`old_string` introuvable dans {}", abs.display()),
            );
        }
        if count > 1 {
            return ToolOutput::err(
                "edit_file",
                format!(
                    "`old_string` trouvé {count} fois dans {} — précisez un fragment unique",
                    abs.display()
                ),
            );
        }
        let new_content = content.replacen(old_str, new_str, 1);
        match tokio::fs::write(&abs, &new_content).await {
            Ok(_) => ToolOutput::ok("edit_file", format!("édité {}", abs.display())),
            Err(e) => ToolOutput::err(
                "edit_file",
                format!("échec écriture {}: {e}", abs.display()),
            ),
        }
    }
}
