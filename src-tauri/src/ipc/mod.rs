//! Handlers Tauri (commands + events) câblés avec le cœur applicatif.
//!
//! J3 : `session_send` fait tourner la boucle d'agent (tool calls + gateway
//! permissions + AGENTS.md). `permission_respond` débloque la gateway.
//! J4 : `model_download` lance le downloader async, `model_list_installed`
//! lit le registry persistant. Voir `docs/ROADMAP.md`.

use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use tauri::State;
use tauri::Emitter;

use crate::config::ConfigManager;
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
    pub config: Arc<ConfigManager>,
}

impl AppState {
    /// Construit l'état partagé. Ouvre (ou crée) le registry et la DB
    /// de sessions au passage, et recharge les sessions historiques.
    pub async fn init() -> anyhow::Result<Self> {
        let registry = Registry::open_default().await?;
        let persistence = crate::sessions::persistence::open_default().await?;
        let sessions = SessionManager::with_persistence(persistence);
        sessions.restore_all().await?;
        let config = ConfigManager::open().await;
        Ok(Self {
            sessions: Arc::new(sessions),
            gateway: Arc::new(Gateway::new()),
            registry: Arc::new(registry),
            downloads: Arc::new(DownloadManager::new()),
            config: Arc::new(config),
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

// ===== API Keys (keyring OS) =====

#[tauri::command]
pub fn provider_set_api_key(provider: String, api_key: String) -> Result<(), String> {
    crate::secrets::set_key(&provider, &api_key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn provider_get_api_key(provider: String) -> Result<Option<String>, String> {
    Ok(crate::secrets::get_key(&provider))
}

#[tauri::command]
pub fn provider_has_api_key(provider: String) -> Result<bool, String> {
    Ok(crate::secrets::has_key(&provider))
}

#[tauri::command]
pub fn provider_delete_api_key(provider: String) -> Result<(), String> {
    crate::secrets::delete_key(&provider).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn provider_list_configured() -> Result<Vec<String>, String> {
    let providers = ["openai", "anthropic", "gemini", "openai_compat"];
    Ok(providers
        .iter()
        .filter(|p| crate::secrets::has_key(p))
        .map(|p| p.to_string())
        .collect())
}

// ===== Ollama — list & pull =====

/// Information sur un modèle Ollama installé localement.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: u64,
    pub digest: String,
    pub modified_at: String,
}

/// Liste les modèles installés localement côté Ollama (`GET /api/tags`).
/// Renvoie une erreur si Ollama n'est pas joignable.
#[tauri::command]
pub async fn ollama_list_models(
) -> Result<Vec<OllamaModelInfo>, String> {
    // On utilise le provider Ollama pour récupérer l'endpoint.
    let endpoint = crate::providers::ollama::DEFAULT_ENDPOINT;
    let url = format!("{}/api/tags", endpoint);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&url).send().await.map_err(|e| {
        format!(
            "Ollama injoignable sur {endpoint} — vérifiez qu'Ollama tourne (`ollama serve`). Détail: {e}"
        )
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama a répondu {status}: {text}"));
    }

    #[derive(serde::Deserialize)]
    struct OllamaTagsResponse {
        models: Vec<OllamaModelEntry>,
    }
    #[derive(serde::Deserialize)]
    struct OllamaModelEntry {
        name: String,
        size: u64,
        digest: String,
        modified_at: String,
    }

    let body: OllamaTagsResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body
        .models
        .into_iter()
        .map(|m| OllamaModelInfo {
            name: m.name,
            size: m.size,
            digest: m.digest,
            modified_at: m.modified_at,
        })
        .collect())
}

/// Lance le pull d'un modèle Ollama (`POST /api/pull`). Le pull est
/// async et émet des events `ollama:pull:progress` / `done` / `error`.
/// Retourne immédiatement après avoir lancé le tokio::task.
#[tauri::command]
pub async fn ollama_pull_model(
    app: tauri::AppHandle,
    model: String,
) -> Result<(), String> {
    let endpoint = crate::providers::ollama::DEFAULT_ENDPOINT.to_string();
    let model_clone = model.clone();

    tokio::spawn(async move {
        let url = format!("{}/api/pull", endpoint);
        let body = serde_json::json!({ "model": model_clone, "stream": true });

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600)) // pull peut être long
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit("ollama:pull:error", serde_json::json!({
                    "model": model_clone,
                    "error": e.to_string(),
                }));
                return;
            }
        };

        let resp = match client.post(&url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = app.emit("ollama:pull:error", serde_json::json!({
                    "model": model_clone,
                    "error": format!("Ollama injoignable: {e}"),
                }));
                return;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let _ = app.emit("ollama:pull:error", serde_json::json!({
                "model": model_clone,
                "error": format!("Ollama a répondu {status}: {text}"),
            }));
            return;
        }

        let mut stream = resp.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();
        let mut last_emit = std::time::Instant::now();

        while let Some(chunk_res) = stream.next().await {
            match chunk_res {
                Ok(bytes) => {
                    buffer.extend_from_slice(&bytes);
                    while let Some(nl) = buffer.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = buffer.drain(..=nl).collect();
                        let line_str = match std::str::from_utf8(&line) {
                            Ok(s) => s.trim(),
                            Err(_) => continue,
                        };
                        if line_str.is_empty() { continue; }
                        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line_str) else { continue };

                        // Throttle: émet un event toutes les 200ms max.
                        let now = std::time::Instant::now();
                        let should_emit = now.duration_since(last_emit) >= std::time::Duration::from_millis(200)
                            || parsed.get("completed").and_then(|v| v.as_bool()) == Some(true);

                        if should_emit {
                            last_emit = now;
                            let _ = app.emit("ollama:pull:progress", &parsed);
                        }

                        // Si completed = true, le pull est fini.
                        if parsed.get("completed").and_then(|v| v.as_bool()) == Some(true) {
                            let _ = app.emit("ollama:pull:done", serde_json::json!({
                                "model": model_clone,
                            }));
                            return;
                        }
                    }
                }
                Err(e) => {
                    let _ = app.emit("ollama:pull:error", serde_json::json!({
                        "model": model_clone,
                        "error": format!("Flux interrompu: {e}"),
                    }));
                    return;
                }
            }
        }

        // Stream terminé sans "completed: true" — considéré comme terminé.
        let _ = app.emit("ollama:pull:done", serde_json::json!({
            "model": model_clone,
        }));
    });

    Ok(())
}

// ===== Config par projet =====

/// Retourne la config globale courante (sérialisée en JSON pour le frontend).
#[tauri::command]
pub async fn config_get(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.get().await;
    serde_json::to_value(&cfg).map_err(|e| e.to_string())
}

/// Charge et merge la config d'un workspace spécifique. Renvoie la config
/// resultante (globale + override workspace).
#[tauri::command]
pub async fn config_get_workspace(
    workspace: String,
) -> Result<serde_json::Value, String> {
    let global = ConfigManager::open().await.get().await;
    let ws = ConfigManager::load_workspace_config(&workspace)
        .await
        .unwrap_or_default();
    let merged = ConfigManager::merge(&global, &ws);
    serde_json::to_value(&merged).map_err(|e| e.to_string())
}

/// Met à jour le provider par défaut dans la config globale.
#[tauri::command]
pub async fn config_set_default_provider(
    state: State<'_, AppState>,
    provider: Option<String>,
) -> Result<(), String> {
    state.config.set_default_provider(provider).await.map_err(|e| e.to_string())
}

/// Met à jour le modèle par défaut dans la config globale.
#[tauri::command]
pub async fn config_set_default_model(
    state: State<'_, AppState>,
    model: Option<String>,
) -> Result<(), String> {
    state.config.set_default_model(model).await.map_err(|e| e.to_string())
}

/// Met à jour l'endpoint Ollama dans la config globale.
#[tauri::command]
pub async fn config_set_ollama_endpoint(
    state: State<'_, AppState>,
    endpoint: Option<String>,
) -> Result<(), String> {
    state.config.set_ollama_endpoint(endpoint).await.map_err(|e| e.to_string())
}

/// Met à jour un override de permission pour un outil.
#[tauri::command]
pub async fn config_set_permission(
    state: State<'_, AppState>,
    tool: String,
    policy: String,
) -> Result<(), String> {
    state
        .config
        .set_permission_override(tool, policy)
        .await
        .map_err(|e| e.to_string())
}

/// Supprime un override de permission.
#[tauri::command]
pub async fn config_remove_permission(
    state: State<'_, AppState>,
    tool: String,
) -> Result<(), String> {
    state
        .config
        .remove_permission_override(&tool)
        .await
        .map_err(|e| e.to_string())
}

// ===== Indexation sémantique (J8) =====

/// Pool SQLite pour l'index sémantique (partagé entre commandes).
pub struct IndexPool(pub sqlx::sqlite::SqlitePool);

/// Initialise la DB d'indexation au démarrage.
async fn open_index_pool() -> Result<sqlx::sqlite::SqlitePool, sqlx::Error> {
    let db_path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cyonima")
        .join("index")
        .join("search.db");
    std::fs::create_dir_all(db_path.parent().unwrap()).ok();
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await?;
    crate::indexing::indexer::init_db(&pool).await?;
    Ok(pool)
}

/// Résultat de recherche sémantique (sérialisé pour le frontend).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub score: f32,
}

/// Lance l'indexation d'un workspace.
#[tauri::command]
pub async fn index_build(
    workspace: String,
) -> Result<crate::indexing::indexer::IndexStats, String> {
    let pool = open_index_pool().await.map_err(|e| e.to_string())?;
    crate::indexing::indexer::clear_index(&pool)
        .await
        .map_err(|e| e.to_string())?;
    crate::indexing::indexer::index_workspace(&pool, std::path::Path::new(&workspace))
        .await
        .map_err(|e| e.to_string())
}

/// Recherche sémantique dans l'index.
#[tauri::command]
pub async fn index_search(
    _workspace: String,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    let pool = open_index_pool().await.map_err(|e| e.to_string())?;
    let mut embedder =
        crate::indexing::embedder::Embedder::load().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(10);
    let results = crate::indexing::search::search(&pool, &mut embedder, &query, limit)
        .await
        .map_err(|e| e.to_string())?;
    Ok(results
        .into_iter()
        .map(|r| SearchResult {
            file_path: r.file_path,
            start_line: r.start_line,
            end_line: r.end_line,
            text: r.text,
            score: r.score,
        })
        .collect())
}

/// Nombre de chunks dans l'index.
#[tauri::command]
pub async fn index_count() -> Result<usize, String> {
    let pool = open_index_pool().await.map_err(|e| e.to_string())?;
    crate::indexing::search::count_chunks(&pool)
        .await
        .map_err(|e| e.to_string())
}
