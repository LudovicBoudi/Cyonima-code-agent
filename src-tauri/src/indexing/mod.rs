//! Indexation sémantique du workspace via l'embedder local.
//!
//! Pipeline : fichier → chunks → embeddings (candle BERT) → SQLite → recherche.
//!
//! L'embedder `all-MiniLM-L6-v2` (384 dims, 512 tokens max) est chargé via
//! safetensors depuis un cache local (`~/.cyonima/models/embedder/`). Les
//! poids sont téléchargés au premier lancement puis réutilisés hors-ligne.

pub mod embedder;
pub mod indexer;
pub mod search;

pub use indexer::blob_to_floats;

/// Dimension des vecteurs d'embedding (all-MiniLM-L6-v2 = 384).
pub const EMBED_DIM: usize = 384;

/// Nombre max de tokens par chunk pour l'embedder.
pub const MAX_TOKENS: usize = 512;

/// Taille d'un chunk en caractères (approximatif).
pub const CHUNK_SIZE: usize = 1000;

/// Chevauchement entre chunks en caractères.
pub const CHUNK_OVERLAP: usize = 200;
