//! Indexeur workspace — parcourt les fichiers, les découpe en chunks,
//! calcule les embeddings et les stocke en SQLite.

use std::path::{Path, PathBuf};

use sqlx::sqlite::SqlitePool;

use super::embedder::{Embedder, EmbedError};
use super::{CHUNK_OVERLAP, CHUNK_SIZE};

/// Un chunk de fichier avec son chemin et numéro de ligne.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
}

/// Résultat d'indexation.
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    pub files_scanned: usize,
    pub chunks_created: usize,
    pub chunks_embedded: usize,
    pub errors: Vec<String>,
}

/// Initialise la table SQLite pour les embeddings.
pub async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            text TEXT NOT NULL,
            embedding BLOB NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path)",
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Supprime l'index existant pour un workspace.
pub async fn clear_index(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM chunks").execute(pool).await?;
    Ok(())
}

/// Parcourt un répertoire et retourne les fichiers texte à indexer.
pub fn walk_workspace(root: &Path, ignore_patterns: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Ignorer les répertoires cachés et binaires.
            !name.starts_with('.')
                && !name.starts_with("node_modules")
                && !name.starts_with("target")
                && !name.starts_with("dist")
                && !ignore_patterns.iter().any(|p| name.contains(p))
        });

    for entry in walker.flatten() {
        if entry.file_type().is_file() {
            let path = entry.path();
            if is_indexable_file(path) {
                files.push(path.to_path_buf());
            }
        }
    }
    files
}

/// Vérifie si un fichier est indexable (texte, pas trop gros).
fn is_indexable_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    matches!(
        ext.as_str(),
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "java" | "c" | "cpp"
            | "h" | "hpp" | "go" | "rb" | "php" | "swift" | "kt" | "cs"
            | "css" | "scss" | "html" | "xml" | "json" | "toml" | "yaml"
            | "yml" | "md" | "txt" | "sql" | "sh" | "bash" | "zsh"
            | "fish" | "ps1" | "bat" | "cmd" | "vue" | "svelte"
    )
}

/// Découpe un fichier en chunks de taille approximative CHUNK_SIZE
/// avec CHUNK_OVERLAP de chevauchement.
pub fn chunk_file(path: &Path, content: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return chunks;
    }

    let mut start = 0;
    while start < lines.len() {
        let mut end = start;
        let mut char_count = 0;

        // Avancer jusqu'à CHUNK_SIZE caractères.
        while end < lines.len() && char_count < CHUNK_SIZE {
            char_count += lines[end].len() + 1; // +1 pour \n
            end += 1;
        }

        // Reculer pour ne pas couper au milieu d'une ligne si possible.
        if end < lines.len() && end > start + 1 {
            end -= 1;
        }

        let text = lines[start..end].join("\n");
        if !text.trim().is_empty() {
            chunks.push(Chunk {
                file_path: path.to_string_lossy().to_string(),
                start_line: start + 1,
                end_line: end,
                text,
            });
        }

        // Avancer avec chevauchement.
        let advance = (end - start).saturating_sub(CHUNK_OVERLAP / 60); // ~60 chars/line
        start += advance.max(1);
    }

    chunks
}

/// Indexe un workspace complet : parcours → chunking → embedding → SQLite.
pub async fn index_workspace(
    pool: &SqlitePool,
    workspace: &Path,
) -> Result<IndexStats, EmbedError> {
    let files = walk_workspace(workspace, &[]);
    let mut stats = IndexStats {
        files_scanned: files.len(),
        ..Default::default()
    };

    // Charger l'embedder.
    let mut embedder = Embedder::load()?;

    // Traiter par batch pour la performance.
    let batch_size = 32;
    let mut all_chunks = Vec::new();

    for path in &files {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let chunks = chunk_file(path, &content);
                stats.chunks_created += chunks.len();
                all_chunks.extend(chunks);
            }
            Err(e) => {
                stats
                    .errors
                    .push(format!("{}: {e}", path.display()));
            }
        }
    }

    // Embed et stocker par batch.
    for batch in all_chunks.chunks(batch_size) {
        let texts: Vec<&str> = batch.iter().map(|c| c.text.as_str()).collect();
        match embedder.embed_batch(&texts) {
            Ok(embeddings) => {
                for (chunk, embedding) in batch.iter().zip(embeddings.iter()) {
                    let blob = floats_to_blob(embedding);
                    sqlx::query(
                        "INSERT INTO chunks (file_path, start_line, end_line, text, embedding)
                         VALUES (?, ?, ?, ?, ?)",
                    )
                    .bind(&chunk.file_path)
                    .bind(chunk.start_line as i64)
                    .bind(chunk.end_line as i64)
                    .bind(&chunk.text)
                    .bind(&blob)
                    .execute(pool)
                    .await
                    .map_err(|e| EmbedError::Inference(format!("insert SQLite: {e}")))?;
                    stats.chunks_embedded += 1;
                }
            }
            Err(e) => {
                stats.errors.push(format!("batch embed: {e}"));
            }
        }
    }

    Ok(stats)
}

/// Convertit un vecteur f32 en blob SQLite (little-endian).
fn floats_to_blob(v: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(v.len() * 4);
    for &f in v {
        blob.extend_from_slice(&f.to_le_bytes());
    }
    blob
}

/// Convertit un blob SQLite en vecteur f32.
pub fn blob_to_floats(blob: &[u8]) -> Vec<f32> {
    blob.chunks(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_file_basic() {
        let content = "line1\nline2\nline3\nline4\nline5\n";
        let chunks = chunk_file(Path::new("test.rs"), content);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].file_path, "test.rs");
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn chunk_file_empty() {
        let chunks = chunk_file(Path::new("empty.rs"), "");
        assert!(chunks.is_empty());
    }

    #[test]
    fn is_indexable_extensions() {
        assert!(is_indexable_file(Path::new("main.rs")));
        assert!(is_indexable_file(Path::new("app.tsx")));
        assert!(is_indexable_file(Path::new("style.css")));
        assert!(!is_indexable_file(Path::new("image.png")));
        assert!(!is_indexable_file(Path::new("binary.exe")));
    }

    #[test]
    fn blob_roundtrip() {
        let original = vec![1.0, 2.5, -42.0, 0.0];
        let blob = floats_to_blob(&original);
        let restored = blob_to_floats(&blob);
        assert_eq!(original.len(), restored.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }
}
