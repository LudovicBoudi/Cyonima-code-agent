//! Outil `grep` — recherche de contenu par regex dans le workspace.
//!
//! Je n'appelle pas ripgrep (binaire externe pas toujours présent en
//! développement Windows) : on scanne les fichiers directement, avec
//! `walkdir` + `regex`. Suffisant pour le périmètre J3 ; l'optimisation
//! mémoire viendra avec l'indexation (J8).

use std::path::Path;

use async_trait::async_trait;
use regex::Regex;
use walkdir::WalkDir;

use super::{truncate_for_llm, Tool, ToolOutput, ToolSpec};

pub struct Grep;

#[async_trait]
impl Tool for Grep {
    fn name(&self) -> &str {
        "grep"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "grep".into(),
            description: "Cherche une regex dans tous les fichiers du workspace. Renvoie les matches (fichier:ligne:texte).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex à chercher" },
                    "include": { "type": "string", "description": "Pattern glob de fichiers à inclure (ex: *.rs)" },
                    "max": { "type": "integer", "description": "Nombre max de matches (défaut 50)" }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value, workspace: &Path) -> ToolOutput {
        let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) else {
            return ToolOutput::err("grep", "argument `pattern` manquant");
        };
        let include = args.get("include").and_then(|v| v.as_str());
        let max = args.get("max").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
        let re = match Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => return ToolOutput::err("grep", format!("regex invalide: {e}")),
        };
        let include_re = match include {
            Some(p) => match Regex::new(p) {
                Ok(r) => Some(r),
                Err(e) => return ToolOutput::err("grep", format!("regex `include` invalide: {e}")),
            },
            None => None,
        };
        let canonical_workspace = match workspace.canonicalize() {
            Ok(p) => p,
            Err(e) => return ToolOutput::err("grep", format!("workspace injoignable: {e}")),
        };
        // Climbing du workspace en tâche async bloquante, donc block_in_place.
        let results: Vec<String> = tokio::task::block_in_place(|| {
            let mut matches = Vec::new();
            for entry in WalkDir::new(&canonical_workspace)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    // Garde la racine ; skip les dossiers bruyants.
                    if e.depth() == 0 {
                        return true;
                    }
                    let Some(name) = e.file_name().to_str() else {
                        return true;
                    };
                    if e.file_type().is_dir() {
                        if name.starts_with('.') {
                            return false;
                        }
                        if matches!(name, "node_modules" | "target" | "dist" | "build") {
                            return false;
                        }
                    }
                    true
                })
            {
                let Ok(entry) = entry else { continue };
                if !entry.file_type().is_file() {
                    continue;
                }
                // Filtre par include pour le basename.
                let basename = entry.file_name().to_string_lossy();
                if let Some(re_inc) = &include_re {
                    if !re_inc.is_match(&basename) {
                        continue;
                    }
                }
                // Lecture binaire : skip si NUL dans les premiers KB.
                let path = entry.path();
                let Ok(content) = std::fs::read_to_string(path) else {
                    continue;
                };
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        let relative = path.strip_prefix(&canonical_workspace).unwrap_or(path);
                        matches.push(format!("{}:{}:{}", relative.display(), i + 1, line.trim()));
                        if matches.len() >= max {
                            return matches;
                        }
                    }
                }
            }
            matches
        });
        let output = if results.is_empty() {
            "Aucun match.".into()
        } else {
            truncate_for_llm(&results.join("\n"), 8_192)
        };
        ToolOutput::ok("grep", format!("{} match(es):\n{output}", results.len()))
    }
}
