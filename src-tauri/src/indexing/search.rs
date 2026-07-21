//! Recherche sémantique — embed une requête et trouve les chunks les plus
//! proches dans l'index SQLite via cosine similarity.

use sqlx::sqlite::SqlitePool;

use super::blob_to_floats;
use super::embedder::{cosine_similarity, EmbedError, Embedder};

/// Résultat de recherche sémantique.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub score: f32,
}

/// Recherche sémantique dans l'index.
pub async fn search(
    pool: &SqlitePool,
    embedder: &mut Embedder,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, EmbedError> {
    // Embed la requête.
    let query_embedding = embedder.embed(query)?;

    // Récupérer tous les chunks de la base.
    let rows: Vec<(i64, String, i64, i64, String, Vec<u8>)> =
        sqlx::query_as("SELECT id, file_path, start_line, end_line, text, embedding FROM chunks")
            .fetch_all(pool)
            .await
            .map_err(|e| EmbedError::Inference(format!("select chunks: {e}")))?;

    // Calculer les scores de similarité.
    let mut results: Vec<SearchResult> = rows
        .into_iter()
        .filter_map(|(_id, file_path, start_line, end_line, text, blob)| {
            let embedding = blob_to_floats(&blob);
            if embedding.len() != query_embedding.len() {
                return None;
            }
            let score = cosine_similarity(&query_embedding, &embedding);
            Some(SearchResult {
                file_path,
                start_line: start_line as usize,
                end_line: end_line as usize,
                text,
                score,
            })
        })
        .collect();

    // Trier par score décroissant.
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);

    Ok(results)
}

/// Nombre de chunks dans l'index.
pub async fn count_chunks(pool: &SqlitePool) -> Result<usize, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM chunks")
        .fetch_one(pool)
        .await?;
    Ok(row.0 as usize)
}
