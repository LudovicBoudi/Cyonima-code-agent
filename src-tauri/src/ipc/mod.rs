//! Handlers Tauri (commands + events) câblés avec le cœur applicatif.
//!
//! J1 : `session_create` / `session_send` / `session_cancel` / `session_fork`
//! sont **réellement opérationnels** via le provider `ollama`. Voir
//! `docs/ROADMAP.md` pour les jalons des autres handlers.

use std::sync::Arc;

use tauri::State;

use crate::hardware::{self, HardwareInfo};
use crate::models::ModelInfo;
use crate::providers::ProviderKind;
use crate::sessions::{SessionInfo, SessionManager};

/// État partagé Tauri.
pub struct AppState {
    pub sessions: Arc<SessionManager>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            sessions: Arc::new(SessionManager::new()),
        }
    }
}

#[tauri::command]
pub fn session_create(
    state: State<'_, AppState>,
    workspace: String,
    model_id: String,
    provider_id: ProviderKind,
) -> SessionInfo {
    state
        .sessions
        .create(workspace, model_id, provider_id, None)
}

#[tauri::command]
pub async fn session_send(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> Result<(), String> {
    state.sessions.send(app, session_id, message)
}

#[tauri::command]
pub fn session_cancel(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state.sessions.cancel(&session_id)
}

#[tauri::command]
pub fn session_fork(state: State<'_, AppState>, session_id: String) -> Option<SessionInfo> {
    state.sessions.fork(&session_id)
}

#[tauri::command]
pub fn session_list(state: State<'_, AppState>) -> Vec<SessionInfo> {
    state.sessions.list()
}

#[tauri::command]
pub fn model_list_installed() -> Vec<ModelInfo> {
    crate::models::list_installed()
}

#[tauri::command]
pub fn model_catalog_list() -> Vec<ModelInfo> {
    crate::models::list_catalog()
}

#[tauri::command]
pub fn model_download(model_id: String, ram_min_gb: Option<u32>) -> Result<(), String> {
    // Garde-fou hardware : on refuse le téléchargement avant d'écrire sur disque.
    let hw = hardware::detect();
    let required = ram_min_gb.unwrap_or(0);
    if required > 0 && !hw.can_run_model(required) {
        return Err(format!(
            "RAM insuffisante pour '{}': requis {} Go, disponible {} Go (marge 1 Go pour l'OS). Téléchargement bloqué.",
            model_id, required, hw.total_ram_gb
        ));
    }
    // J4 : downloader async + events `model:download:progress`.
    Ok(())
}

#[tauri::command]
pub fn model_download_cancel(_model_id: String) {
    // J4 : cancellation token partagé.
}

#[tauri::command]
pub fn model_import_custom(path: String) -> Result<ModelInfo, String> {
    crate::models::import_custom(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn permission_respond(_request_id: String, _decision: String) {
    // J3 : débloque le gateway permissions en attente.
}

/// Snapshot hardware pour l'UI (affiché dans Settings + barre de statut).
#[tauri::command]
pub fn hardware_get() -> HardwareInfo {
    hardware::detect()
}

/// Vérifie qu'un modèle demandant `ram_min_gb` peut tourner sur cette machine.
#[tauri::command]
pub fn hardware_can_run_model(ram_min_gb: u32) -> bool {
    hardware::detect().can_run_model(ram_min_gb)
}
