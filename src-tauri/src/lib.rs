//! Cœur applicatif Cyonima-code-agent.
//!
//! Point d'entrée monté par le binaire Tauri. Toute la logique métier vit
//! dans les sous-modules ; ce fichier ne fait que câbler le runtime Tauri
//! avec les handlers IPC.

pub mod config;
pub mod hardware;
pub mod indexing;
pub mod ipc;
pub mod models;
pub mod permissions;
pub mod providers;
pub mod sessions;
pub mod tools;

use tracing_subscriber::EnvFilter;

/// Monte l'application Tauri.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let app_state = ipc::AppState::default();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            ipc::session_create,
            ipc::session_send,
            ipc::session_cancel,
            ipc::session_fork,
            ipc::session_list,
            ipc::model_list_installed,
            ipc::model_catalog_list,
            ipc::model_download,
            ipc::model_download_cancel,
            ipc::model_import_custom,
            ipc::permission_respond,
            ipc::hardware_get,
            ipc::hardware_can_run_model,
        ])
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application Tauri");
}
