//! Handlers Tauri (commands + events) câblés avec le cœur applicatif.
//!
//! J3 : `session_send` fait tourner la boucle d'agent complète (tool calls +
//! gateway permissions + AGENTS.md). `permission_respond` débloque la gateway
//! via IPC. Voir `docs/ROADMAP.md` pour les autres jalons.

use std::sync::Arc;

use tauri::State;

use crate::hardware::{self, HardwareInfo};
use crate::models::ModelInfo;
use crate::permissions::{Decision, Gateway};
use crate::providers::ProviderKind;
use crate::sessions::{SessionInfo, SessionManager};

/// État partagé Tauri.
pub struct AppState {
    pub sessions: Arc<SessionManager>,
    pub gateway: Arc<Gateway>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            sessions: Arc::new(SessionManager::new()),
            gateway: Arc::new(Gateway::new()),
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
    state.sessions.create(
        Arc::clone(&state.gateway),
        workspace,
        model_id,
        provider_id,
        None,
    )
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
}

#[tauri::command]
pub fn session_cancel(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state.sessions.cancel(&session_id)
}

#[tauri::command]
pub fn session_fork(state: State<'_, AppState>, session_id: String) -> Option<SessionInfo> {
    state.sessions.fork(Arc::clone(&state.gateway), &session_id)
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

/// Débloque la gateway en attente pour un tool-call précédemment demandé.
/// Appelée par l'UI après qu'un utilisateur a cliqué Allow/Deny sur la
/// modale d'approbation.
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
