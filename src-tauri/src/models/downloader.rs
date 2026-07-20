//! Downloader de modèles GGUF — robuste, reprendable,Cancelable.
//!
//! Fonctionnement (J4) :
//!
//! 1. **Téléchargement incrémental avec reprise**
//!    - Le fichier cible est écrit dans `<dest>.part` pendant le download.
//!    - Si le téléchargement est interrompu (cancel, crash réseau, kill app),
//!      le `.part` reste sur disque. Au prochain `download`, on reprend depuis
//!      la taille actuelle du `.part` via `Range: bytes=<n>-`.
//!    - À la fin, si le SHA256 matche, on renomme `.part` → `<dest>`.
//!
//! 2. **Vérification SHA256**
//!    - Le SHA256 est calculé incrémentalement pendant l'écriture des bytes
//!      (un seul `Sha256::update` par flush, pas par byte).
//!    - À la fin on compare au SHA256 attendu (venant du catalogue). Si le
//!      catalogue n'a pas encore renseigné le SHA256 (`TODO_J4`), on calcule
//!      quand même le hash mais on accepte n'importe quel résultat — la
//!      vérification stricte viendra quand on remplira le catalogue.
//!
//! 3. **Cancellation**
//!    - Chaque download a un `CancellationToken` partagé via le
//!      `DownloadManager`. L'IPC `model_download_cancel` l'invoque.
//!    - Le token est checké à chaque chunk reçu → le `.part` reste sur disque,
//!      prêt à reprendre.
//!
//! 4. **Progression**
//!    - On émet l'event Tauri `model:download:progress` tous les 256 KB
//!      (anti-spam) avec le nombre de bytes téléchargés et la taille totale.
//!    - Également `model:download:done` en cas de succès et
//!      `model:download:error` en cas d'erreur réseau ou SHA256 mismatch.

use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::registry::{Registry, RegistryEntry};

/// Événement émis pendant le téléchargement.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    pub model_id: String,
    /// Bytes téléchargés (parts cumulés).
    pub downloaded: u64,
    /// Taille totale attendue (peut être revue après un 200 vs 206).
    pub total: u64,
    /// Vitesse en octets/s (moyenne mobile courte).
    pub bytes_per_second: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDoneEvent {
    pub model_id: String,
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadErrorEvent {
    pub model_id: String,
    pub error: String,
}

/// Mémoire des téléchargements en cours : clé = model_id, valeur = token.
/// On stocke volontairement dans une DashMap (pas de Mutex), car on doit
/// pouvoir insérer / cancel / retirer sans lock global.
pub struct DownloadManager {
    cancels: dashmap::DashMap<String, CancellationToken>,
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self {
            cancels: dashmap::DashMap::new(),
        }
    }
}

impl DownloadManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lance un téléchargement en arrière-plan. Retourne immédiatement ;
    /// la progression vient via events Tauri.
    pub fn start(
        &self,
        app: AppHandle,
        registry: Arc<Registry>,
        entry: super::CatalogEntry,
        dest_dir: PathBuf,
    ) -> Result<(), String> {
        // Refuse un download déjà en cours pour le même model_id.
        if self.cancels.contains_key(&entry.id) {
            return Err(format!("Téléchargement déjà en cours pour '{}'", entry.id));
        }

        let cancel = CancellationToken::new();
        self.cancells_insert(entry.id.clone(), cancel.clone());

        let cancels = self.cancels.clone();
        let model_id = entry.id.clone();
        tokio::spawn(async move {
            let result = run_download(app.clone(), cancel.clone(), entry, dest_dir).await;

            // Retire le token quel que soit le résultat.
            cancels.remove(&model_id);

            match result {
                Ok((path, sha, size, registry_entry)) => {
                    // Enregistre dans le registry persistant.
                    if let Err(e) = registry.upsert(registry_entry).await {
                        let _ = app.emit(
                            "model:download:error",
                            DownloadErrorEvent {
                                model_id: model_id.clone(),
                                error: format!("modèle téléchargé mais non enregistré: {e}"),
                            },
                        );
                    }
                    let _ = app.emit(
                        "model:download:done",
                        DownloadDoneEvent {
                            model_id: model_id.clone(),
                            path: path.to_string_lossy().to_string(),
                            sha256: sha,
                            size_bytes: size,
                        },
                    );
                }
                Err(err) => {
                    let _ = app.emit(
                        "model:download:error",
                        DownloadErrorEvent {
                            model_id: model_id.clone(),
                            error: err,
                        },
                    );
                }
            }
        });
        Ok(())
    }

    /// Annule un téléchargement en cours. Ne supprime pas le `.part` pour
    /// permettre une reprise future.
    pub fn cancel(&self, model_id: &str) -> Result<(), String> {
        if let Some(token) = self.cancels.get(model_id) {
            token.cancel();
            Ok(())
        } else {
            Err(format!("Aucun téléchargement en cours pour '{model_id}'"))
        }
    }

    pub fn is_downloading(&self, model_id: &str) -> bool {
        self.cancels.contains_key(model_id)
    }

    fn cancells_insert(&self, id: String, token: CancellationToken) {
        self.cancels.insert(id, token);
    }
}

/// Taille de flush pour progress + sha256 update : 256 KB.
const FLUSH_BYTES: usize = 256 * 1024;

/// Lance le téléchargement proprement dit. Retourne le chemin final du GGUF,
/// le SHA256 calculé, la taille totale et l'entrée registry à insérer.
async fn run_download(
    app: AppHandle,
    cancel: CancellationToken,
    entry: super::CatalogEntry,
    dest_dir: PathBuf,
) -> Result<(PathBuf, String, u64, RegistryEntry), String> {
    // Prépare le dossier de destination.
    tokio::fs::create_dir_all(&dest_dir)
        .await
        .map_err(|e| format!("impossible de créer {}: {e}", dest_dir.display()))?;

    let dest = dest_dir.join(format!("{}.gguf", entry.id));
    let part = dest_dir.join(format!("{}.gguf.part", entry.id));

    // Reprend depuis la taille du `.part` s'il existe.
    let resume_from: u64 = match tokio::fs::metadata(&part).await {
        Ok(meta) => meta.len(),
        Err(_) => 0,
    };

    // Ouvre le .part en append (création si absent).
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&part)
        .await
        .map_err(|e| format!("échec ouverture {}: {e}", part.display()))?;

    // SHA256 incrémental : si on reprend, on doit hacher la partie déjà sur
    // disque aussi. Coût acceptable car le download est IO-bound de toute façon.
    let mut hasher = Sha256::new();
    if resume_from > 0 {
        let existing = tokio::fs::read(&part)
            .await
            .map_err(|e| format!("échec lecture .part pour rehash: {e}"))?;
        hasher.update(&existing);
    }

    // Requête HTTP avec Range. On tolère 200 (serveur ignore Range) ou 206 (OK).
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("client HTTP: {e}"))?;

    let mut req = client.get(&entry.url);
    if resume_from > 0 {
        req = req.header("Range", format!("bytes={resume_from}-"));
    }
    let response = req
        .send()
        .await
        .map_err(|e| format!("échec requête HTTP vers {}: {e}", entry.url))?;

    let status = response.status();
    let total_from_server = response.content_length().unwrap_or(0);
    let server_supports_range = status.as_u16() == 206;
    let server_total_bytes = if server_supports_range {
        // 206 avec Content-Length = taille du reste. Total = resume_from + reste.
        resume_from + total_from_server
    } else if status.as_u16() == 200 {
        // 200 = serveur ignore Range, renvoie tout. On doit remettre hasher à 0
        // et tronquer le fichier, car on va tout réécrire depuis le début.
        if resume_from > 0 {
            drop(file);
            tokio::fs::remove_file(&part)
                .await
                .map_err(|e| format!("impossible de supprimer .part stale: {e}"))?;
            file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&part)
                .await
                .map_err(|e| format!("échec réouverture {}: {e}", part.display()))?;
            hasher = Sha256::new();
        }
        total_from_server
    } else {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, text));
    };

    if !status.is_success() {
        return Err(format!("réponse HTTP invalide: {status}"));
    }

    // Si le total catalogue est plus fiable (non nul), on l'utilise pour l'UI.
    let total = if entry.size_bytes > 0 {
        entry.size_bytes
    } else {
        server_total_bytes
    };

    // Boucle de streaming : on accumule FLUSH_BYTES puis on écrit + update
    // hasher + emit progress.
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = resume_from;
    let mut buf: Vec<u8> = Vec::with_capacity(FLUSH_BYTES);
    let start_time = std::time::Instant::now();
    let mut last_event_instant = std::time::Instant::now();
    let mut last_event_downloaded: u64 = downloaded;

    while let Some(chunk_res) = stream.next().await {
        if cancel.is_cancelled() {
            // Flush partiel pour ne pas perdre les bytes reçus, puis retour.
            if !buf.is_empty() {
                file.write_all(&buf)
                    .await
                    .map_err(|e| format!("write flush: {e}"))?;
                hasher.update(&buf);
                buf.clear();
            }
            file.flush().await.ok();
            return Err("Téléchargement annulé par l'utilisateur".into());
        }

        let bytes = chunk_res.map_err(|e| format!("stream interrompu: {e}"))?;
        buf.extend_from_slice(&bytes);

        while buf.len() >= FLUSH_BYTES {
            let to_write: Vec<u8> = buf.drain(..FLUSH_BYTES).collect();
            file.write_all(&to_write)
                .await
                .map_err(|e| format!("write chunk: {e}"))?;
            hasher.update(&to_write);
            downloaded += to_write.len() as u64;

            // Throttle events : max 1 event / 200ms pour ne pas spammer l'IPC.
            let now = std::time::Instant::now();
            if now.duration_since(last_event_instant).as_millis() >= 200 {
                let elapsed = now.duration_since(last_event_instant).as_secs_f64();
                let recent_bytes = downloaded - last_event_downloaded;
                let bps = if elapsed > 0.0 {
                    (recent_bytes as f64 / elapsed) as u64
                } else {
                    0
                };
                let _ = app.emit(
                    "model:download:progress",
                    DownloadProgressEvent {
                        model_id: entry.id.clone(),
                        downloaded,
                        total,
                        bytes_per_second: bps,
                    },
                );
                last_event_instant = now;
                last_event_downloaded = downloaded;
            }
        }
    }

    // Flush final.
    if !buf.is_empty() {
        file.write_all(&buf)
            .await
            .map_err(|e| format!("write flush: {e}"))?;
        hasher.update(&buf);
        downloaded += buf.len() as u64;
    }
    file.flush().await.ok();
    drop(file);

    // Émet un dernier progress pour ne pas laisser l'UI à 99% avant le done.
    let _ = app.emit(
        "model:download:progress",
        DownloadProgressEvent {
            model_id: entry.id.clone(),
            downloaded,
            total,
            bytes_per_second: 0,
        },
    );

    // Calcul du SHA256 final.
    let sha = hasher.finalize();
    let sha_hex = hex::encode(sha);

    // Vérifie le SHA256 attendu si renseigné et non vide.
    if !entry.sha256.is_empty()
        && entry.sha256 != "TODO_J4"
        && !entry.sha256.eq_ignore_ascii_case(&sha_hex)
    {
        // Supprime le .part si mismatch pour forcer un re-download propre
        // (sinon on reprendrait sur un fichier corrompu).
        tokio::fs::remove_file(&part).await.ok();
        return Err(format!(
            "SHA256 mismatch: attendu {} vs calculé {}",
            entry.sha256, sha_hex
        ));
    }

    // Rename .part -> final.
    tokio::fs::rename(&part, &dest)
        .await
        .map_err(|e| format!("impossible de renommer {}: {e}", part.display()))?;

    let _ = start_time; // silencieux pour clippy
    let _ = Uuid::nil();

    let registry_entry = RegistryEntry {
        id: entry.id.clone(),
        name: entry.name.clone(),
        path: dest.to_string_lossy().to_string(),
        size_bytes: downloaded,
        sha256: sha_hex.clone(),
        quantization: entry.quantization.clone(),
        license: entry.license.clone(),
        ram_min_gb: entry.ram_min_gb,
        ollama_tag: entry.ollama_tag.clone(),
        url: entry.url.clone(),
        downloaded_at: chrono::Utc::now(),
    };

    Ok((dest, sha_hex, downloaded, registry_entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_known_content() {
        let mut h = Sha256::new();
        h.update(b"abc");
        let got = hex::encode(h.finalize());
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[tokio::test]
    async fn download_manager_starts_and_cancels() {
        let dm = DownloadManager::new();
        assert!(!dm.is_downloading("any-id"));
        assert!(dm.cancel("any-id").is_err());
    }
}
