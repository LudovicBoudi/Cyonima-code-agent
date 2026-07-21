//! Embedder local via candle BERT (all-MiniLM-L6-v2).
//!
//! Les poids safetensors et le tokenizer sont téléchargés au premier lancement
//! depuis HuggingFace, puis mis en cache dans `~/.cyonima/models/embedder/`.

use std::path::PathBuf;

use candle_core::{DType, Device, Tensor};
use candle_transformers::models::bert::BertModel;
use tokenizers::Tokenizer;

use super::EMBED_DIM;

/// Modèle d'embedding local.
pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

/// Erreur d'embedding.
#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("modèle non disponible: {0}")]
    ModelUnavailable(String),
    #[error("tokenisation: {0}")]
    Tokenization(String),
    #[error("inférence: {0}")]
    Inference(String),
}

impl Embedder {
    /// Charge l'embedder depuis le cache local.
    /// DÉSACTIVÉ : évite les téléchargements automatiques qui déclenchent Windows Defender.
    pub fn load() -> Result<Self, EmbedError> {
        Err(EmbedError::ModelUnavailable(
            "Embedder désactivé temporairement pour éviter les blocages Windows Defender. Les téléchargements automatiques seront réactivés plus tard.".to_string()
        ))
    }

    /// Embed un texte unique. Retourne un vecteur de dimension EMBED_DIM.
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| EmbedError::Tokenization(e.to_string()))?;

        let ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        if ids.is_empty() {
            return Ok(vec![0.0; EMBED_DIM]);
        }

        let input_ids = Tensor::new(ids, &self.device)
            .map_err(|e| EmbedError::Inference(e.to_string()))?
            .unsqueeze(0)
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        let token_type_ids = Tensor::zeros(input_ids.shape(), DType::U32, &self.device)
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        let mask_vec: Vec<f32> = attention_mask.iter().map(|&m| m as f32).collect();
        let attention_mask = Tensor::new(mask_vec.as_slice(), &self.device)
            .map_err(|e| EmbedError::Inference(e.to_string()))?
            .unsqueeze(0)
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        // Forward pass.
        let hidden = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        // Mean pooling.
        let mask_expanded = attention_mask
            .unsqueeze(2)
            .map_err(|e| EmbedError::Inference(e.to_string()))?
            .expand(hidden.shape())
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        let masked_hidden = hidden
            .mul(&mask_expanded)
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        let sum = masked_hidden
            .sum(1)
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        let mask_sum = attention_mask
            .sum(1)
            .map_err(|e| EmbedError::Inference(e.to_string()))?
            .clamp(1e-9, f32::MAX)
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        let pooled = sum
            .div(&mask_sum)
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        // L2 normalize.
        let norm = pooled
            .sqr()
            .map_err(|e| EmbedError::Inference(e.to_string()))?
            .sum(1)
            .map_err(|e| EmbedError::Inference(e.to_string()))?
            .sqrt()
            .map_err(|e| EmbedError::Inference(e.to_string()))?
            .clamp(1e-12, f32::MAX)
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        let normalized = pooled
            .div(&norm)
            .map_err(|e| EmbedError::Inference(e.to_string()))?;

        normalized
            .squeeze(0)
            .map_err(|e| EmbedError::Inference(e.to_string()))?
            .to_vec1::<f32>()
            .map_err(|e| EmbedError::Inference(e.to_string()))
    }

    /// Embed un batch de textes. Retourne un vecteur de vecteurs.
    pub fn embed_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
}

/// Cosine similarity entre deux vecteurs.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a < 1e-9 || norm_b < 1e-9 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Répertoire cache de l'embedder.
/// Conservé pour la réactivation future de l'embedder local (cf `load`).
#[allow(dead_code)]
fn embedder_cache_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cyonima")
        .join("models")
        .join("embedder")
}

/// Télécharge un fichier URL vers un chemin local (async).
/// Conservé pour la réactivation future de l'embedder local (cf `load`).
#[allow(dead_code)]
async fn download_file_async(url: &str, dest: &PathBuf) -> Result<(), String> {
    let response = reqwest::get(url).await.map_err(|e| e.to_string())?;
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    std::fs::write(dest, &bytes).map_err(|e| e.to_string())?;
    Ok(())
}

/// Télécharge un fichier URL vers un chemin local (bloquant, pour usage hors async).
/// Conservé pour la réactivation future de l'embedder local (cf `load`).
#[allow(dead_code)]
fn download_file(url: &str, dest: &PathBuf) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(download_file_async(url, dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_empty() {
        assert!(cosine_similarity(&[], &[]).abs() < 1e-6);
    }
}
