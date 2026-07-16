//! Indexation sémantique du workspace via l'embedder local (< 50 Mo).
//!
//! Cf `docs/ARCHITECTURE.md` — l'embedder GGUF est embarqué via
//! `tauri.bundle.resources`. Les embeddings sont stockés en SQLite.
//!
//! Implémentation concrète au jalon J8. Squelette J0 uniquement.

/// Dummy placeholder : renvoie une dimension standard pour MiniLM-L6 (384).
pub const EMBED_DIM: usize = 384;
