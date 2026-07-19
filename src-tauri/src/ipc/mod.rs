//! Handlers Tauri (commands + events) câblés avec le cœur applicatif.
//!
//! J3 : `session_send` fait tourner la boucle d'agent (tool calls + gateway
//! permissions + AGENTS.md). `permission_respond` débloque la gateway.
//! J4 : `model_download` lance le downloader async, `model_list_installed`
//! lit le registry persistant. Voir `docs/ROADMAP.md`.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;

use crate::hardware::{self, HardwareInfo};
use crate::models::{self, downloader::DownloadManager, registry::Registry, ModelInfo};
use crate::permissions::{Decision, Gateway};
use crate::providers::ProviderKind;
use crate::sessions::{SessionInfo, SessionManager};

/// État partagé Tauri.
pub struct AppState {
    pub sessions: Arc<SessionManager>,
    pub gateway: Arc<Gateway>,
    pub registry: Arc<Registry>,
    pub downloads: Arc<DownloadManager>,
}

impl AppState {
    /// Construit l'état partagé. Ouvre (ou crée) le registry et la DB
    /// de sessions au passage, et recharge les sessions historiques.
    pub async fn init() -> anyhow::Result<Self> {
        let registry = Registry::open_default().await?;
        let persistence = crate::sessions::persistence::open_default().await?;
        let sessions = SessionManager::with_persistence(persistence);
        // Recharge les sessions historiques en cache in-memory.
        sessions.restore_all().await?;
        Ok(Self {
            sessions: Arc::new(sessions),
            gateway: Arc::new(Gateway::new()),
            registry: Arc::new(registry),
            downloads: Arc::new(DownloadManager::new()),
        })
    }
}

#[tauri::command]
pub async fn session_create(
    state: State<'_, AppState>,
    workspace: String,
    model_id: String,
    provider_id: ProviderKind,
) -> Result<SessionInfo, String> {
    Ok(state
        .sessions
        .create(
            Arc::clone(&state.gateway),
            workspace,
            model_id,
            provider_id,
            None,
        )
        .await)
}

#[tauri::command]
pub async fn session_send(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> Result<(), String> {
    state
        .sessions
        .send(app, Arc::clone(&state.gateway), session_id, message)
        .await
}

#[tauri::command]
pub fn session_cancel(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state.sessions.cancel(&session_id)
}

#[tauri::command]
pub async fn session_fork(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Option<SessionInfo>, String> {
    Ok(state
        .sessions
        .fork(Arc::clone(&state.gateway), &session_id)
        .await)
}

/// Supprime définitivement une session (cache in-memory + SQLite + messages).
#[tauri::command]
pub async fn session_delete(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state.sessions.delete(&session_id).await
}

/// Récupère l'historique complet d'une session (utile pour restaurer l'UI au
/// démarrage ou pour rafraîchir une session inactive mise en arrière-plan).
#[tauri::command]
pub async fn session_history(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<crate::providers::ChatMessage>, String> {
    state
        .sessions
        .history(&session_id)
        .await
        .ok_or_else(|| format!("Session '{session_id}' introuvable"))
}

#[tauri::command]
pub fn session_list(state: State<'_, AppState>) -> Vec<SessionInfo> {
    state.sessions.list()
}

#[tauri::command]
pub async fn model_list_installed(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    Ok(models::list_installed(&state.registry).await)
}

#[tauri::command]
pub async fn model_catalog_list(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    Ok(models::list_catalog(&state.registry).await)
}

/// Lance le téléchargement d'un modèle du catalogue. Spawn un tokio::task qui
/// stream les bytes et émet les events `model:download:progress`, `done`,
/// `error`. Retourne immédiatement après le garde-fou hardware.
#[tauri::command]
pub async fn model_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    model_id: String,
) -> Result<(), String> {
    // 1) Trouve l'entrée du catalogue.
    let entry = models::find_catalog_entry(&model_id)
        .ok_or_else(|| format!("Modèle '{model_id}' introuvable dans le catalogue"))?;

    // 2) Garde-fou hardware : refuse avant d'allouer le download.
    let hw = hardware::detect();
    if entry.ram_min_gb > 0 && !hw.can_run_model(entry.ram_min_gb) {
        return Err(format!(
            "RAM insuffisante pour '{}': requis {} Go, disponible {} Go (marge 1 Go pour l'OS). Téléchargement bloqué.",
            model_id, entry.ram_min_gb, hw.total_ram_gb
        ));
    }

    // 3) Détermine le dossier de destination : `~/.cyonima/models/`.
    let dest_dir = default_models_dir();

    // 4) Lance le download.
    state
        .downloads
        .start(app, Arc::clone(&state.registry), entry, dest_dir)
}

#[tauri::command]
pub fn model_download_cancel(state: State<'_, AppState>, model_id: String) -> Result<(), String> {
    state.downloads.cancel(&model_id)
}

#[tauri::command]
pub async fn model_import_custom(
    state: State<'_, AppState>,
    path: String,
) -> Result<ModelInfo, String> {
    let entry = models::validate_custom(&path).map_err(|e| e.to_string())?;
    let info_id = entry.id.clone();
    let info_name = entry.name.clone();
    let info_size = entry.size_bytes;
    let info_quant = entry.quantization.clone();
    let info_license = entry.license.clone();
    let info_ram = entry.ram_min_gb;
    let info_ollama = entry.ollama_tag.clone();
    let info_url = Some(entry.url.clone()).filter(|u| !u.is_empty());
    let info_path = entry.path.clone();
    state
        .registry
        .upsert(entry)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ModelInfo {
        id: info_id,
        name: info_name,
        size_bytes: info_size,
        quantization: info_quant,
        license: info_license,
        ram_min_gb: info_ram,
        installed: true,
        ollama_tag: info_ollama,
        url: info_url,
        installed_path: Some(info_path),
    })
}

/// Débloque la gateway en attente pour un tool-call précédemment demandé.
#[tauri::command]
pub async fn permission_respond(
    state: State<'_, AppState>,
    request_id: String,
    decision: String,
) -> Result<(), String> {
    let decision = match decision.as_str() {
        "allow" => Decision::Allow,
        "deny" => Decision::Deny,
        other => return Err(format!("decision invalide: {other} (attendu: allow|deny)")),
    };
    state.gateway.respond(&request_id, decision).await;
    Ok(())
}

#[tauri::command]
pub fn hardware_get() -> HardwareInfo {
    hardware::detect()
}

#[tauri::command]
pub fn hardware_can_run_model(ram_min_gb: u32) -> bool {
    hardware::detect().can_run_model(ram_min_gb)
}

/// Dossier de stockage des GGUF : `~/.cyonima/models/`. Sera configurable en J9.
fn default_models_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".cyonima").join("models")
}
