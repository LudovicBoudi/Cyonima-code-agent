//! Outil `semantic_search` — recherche sémantique dans le workspace.
//!
//! L'agent peut poser des questions en langage naturel sur le code et obtenir
//! des hits pertinents (fichier, lignes, score).

use std::path::Path;

use async_trait::async_trait;

use super::{Tool, ToolOutput, ToolSpec};

pub struct SemanticSearch;

#[async_trait]
impl Tool for SemanticSearch {
    fn name(&self) -> &str {
        "semantic_search"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "semantic_search".into(),
            description: "Recherche sémantique dans le code source du workspace. \
                Posez des questions en langage naturel pour trouver les fichiers \
                et sections pertinents. Ex: \"où est géré le panier dans le code\""
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Requête de recherche en langage naturel"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Nombre max de résultats (défaut: 5)",
                        "default": 5
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value, _workspace: &Path) -> ToolOutput {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => {
                return ToolOutput::err("semantic_search", "argument 'query' manquant");
            }
        };

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        // Ouvrir la DB d'indexation.
        let db_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("cyonima")
            .join("index")
            .join("search.db");

        if !db_path.exists() {
            return ToolOutput::err(
                "semantic_search",
                "Index non disponible. Demandez d'abord d'indexer le workspace avec index_build.",
            );
        }

        let url = format!("sqlite:{}?mode=ro", db_path.display());
        let pool = match sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                return ToolOutput::err("semantic_search", format!("ouverture index: {e}"));
            }
        };

        // Charger l'embedder.
        let mut embedder = match crate::indexing::embedder::Embedder::load() {
            Ok(e) => e,
            Err(e) => {
                return ToolOutput::err("semantic_search", format!("chargement embedder: {e}"));
            }
        };

        // Rechercher.
        let results =
            match crate::indexing::search::search(&pool, &mut embedder, query, limit).await {
                Ok(r) => r,
                Err(e) => {
                    return ToolOutput::err("semantic_search", format!("recherche: {e}"));
                }
            };

        if results.is_empty() {
            return ToolOutput::ok(
                "semantic_search",
                format!("Aucun résultat pour « {query} »."),
            );
        }

        // Formater les résultats pour le LLM.
        let mut output = format!("{} résultat(s) pour « {query} » :\n\n", results.len());
        for (i, r) in results.iter().enumerate() {
            output.push_str(&format!(
                "{}. {} (lignes {}-{}, score {:.2})\n{}\n\n",
                i + 1,
                r.file_path,
                r.start_line,
                r.end_line,
                r.score,
                truncate_text(&r.text, 300),
            ));
        }

        ToolOutput::ok("semantic_search", output)
    }
}

fn truncate_text(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
