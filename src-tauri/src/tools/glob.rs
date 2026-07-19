//! Outil `glob` — recherche de fichiers par pattern (ex: `**/*.rs`).
//!
//! Implémentation légère qui marche pour des workspaces raisonnablement
//! petits. Pour de très gros codebases, on switchera vers une indexation
//! persistante en J8.

use std::path::Path;

use async_trait::async_trait;
use globwalk::GlobWalkerBuilder;

use super::{truncate_for_llm, Tool, ToolOutput, ToolSpec};

pub struct Glob;

#[async_trait]
impl Tool for Glob {
    fn name(&self) -> &str {
        "glob"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "glob".into(),
            description:
                "Liste les fichiers du workspace correspondant à un pattern glob (ex: **/*.rs)."
                    .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Pattern glob (ex: **/*.rs)" },
                    "max": { "type": "integer", "description": "Nombre max de résultats (défaut 100)" }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value, workspace: &Path) -> ToolOutput {
        let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) else {
            return ToolOutput::err("glob", "argument `pattern` manquant");
        };
        let max = args.get("max").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
        let canonical_workspace = match workspace.canonicalize() {
            Ok(p) => p,
            Err(e) => return ToolOutput::err("glob", format!("workspace injoignable: {e}")),
        };
        // Globwalk standardise le pattern pour ancrer sur le workspace.
        let walker = match GlobWalkerBuilder::new(&canonical_workspace, pattern)
            .follow_links(false)
            .build()
        {
            Ok(w) => w,
            Err(e) => return ToolOutput::err("glob", format!("pattern glob invalide: {e}")),
        };
        let mut paths = Vec::new();
        for entry in walker.into_iter().take(max) {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(&canonical_workspace)
                .unwrap_or(entry.path());
            paths.push(relative.display().to_string());
        }
        paths.sort();
        let output = if paths.is_empty() {
            "Aucun fichier trouvé.".into()
        } else {
            truncate_for_llm(&paths.join("\n"), 8_192)
        };
        ToolOutput::ok("glob", format!("{} fichier(s):\n{output}", paths.len()))
    }
}
