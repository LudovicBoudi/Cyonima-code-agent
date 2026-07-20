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
pub mod secrets;
pub mod sessions;
pub mod tools;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

/// Monte l'application Tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // `AppState::init` est async (ouverture du registry sur disque,
            // création du DownloadManager). On l'exécute via un runtime
            // tokio dédié pour ne pas bloquer le setup thread.
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("échec construction runtime tokio pour AppState::init");
            let app_state = rt
                .block_on(ipc::AppState::init())
                .expect("échec initialisation AppState (registry / downloads)");
            drop(rt);
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::session_create,
            ipc::session_send,
            ipc::session_cancel,
            ipc::session_fork,
            ipc::session_delete,
            ipc::session_history,
            ipc::session_list,
            ipc::model_list_installed,
            ipc::model_catalog_list,
            ipc::model_download,
            ipc::model_download_cancel,
            ipc::model_import_custom,
            ipc::permission_respond,
            ipc::hardware_get,
            ipc::hardware_can_run_model,
            ipc::provider_set_api_key,
            ipc::provider_get_api_key,
            ipc::provider_has_api_key,
            ipc::provider_delete_api_key,
            ipc::provider_list_configured,
            ipc::ollama_list_models,
            ipc::ollama_pull_model,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application Tauri");
}
