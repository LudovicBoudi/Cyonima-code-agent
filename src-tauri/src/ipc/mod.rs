//! Handlers Tauri (commands + events) câblés avec le cœur applicatif.
//!
//! J3 : `session_send` fait tourner la boucle d'agent (tool calls + gateway
//! permissions + AGENTS.md). `permission_respond` débloque la gateway.
//! J4 : `model_download` lance le downloader async, `model_list_installed`
//! lit le registry persistant. Voir `docs/ROADMAP.md`.

use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use tauri::Emitter;
use tauri::State;

use crate::config::ConfigManager;
use crate::hardware::{self, HardwareInfo};
use crate::models::{self, registry::Registry, ModelInfo};
use crate::permissions::{Decision, Gateway};
use crate::providers::ProviderKind;
use crate::sessions::{SessionInfo, SessionManager};

/// État partagé Tauri.
pub struct AppState {
    pub sessions: Arc<SessionManager>,
    pub gateway: Arc<Gateway>,
    pub registry: Arc<Registry>,
    // pub downloads: Arc<DownloadManager>, // OBSOLÈTE - utiliser Ollama
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
            // downloads: Arc::new(DownloadManager::new()), // OBSOLÈTE
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
    model: Option<String>,
) -> Result<(), String> {
    state
        .sessions
        .send(app, Arc::clone(&state.gateway), session_id, message, model)
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

/// Lance le téléchargement d'un modèle du catalogue. 
/// OBSOLÈTE : utiliser Ollama pull à la place.
#[tauri::command]
pub async fn model_download(
    _app: tauri::AppHandle,
    _state: State<'_, AppState>,
    _model_id: String,
) -> Result<(), String> {
    Err("Téléchargement GGUF obsolète. Utilisez 'Pull Ollama' dans le catalogue.".to_string())
}

#[tauri::command]
pub fn model_download_cancel(_state: State<'_, AppState>, _model_id: String) -> Result<(), String> {
    Err("Téléchargement GGUF obsolète.".to_string())
}

/// Vide le cache des modèles téléchargés (registry + fichiers GGUF).
/// Utile pour nettoyer après migration vers Ollama.
#[tauri::command]
pub async fn model_clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    // 1) Vider le registry
    state.registry.clear().await.map_err(|e| format!("Erreur lors du nettoyage du registry: {e}"))?;
    
    // 2) Supprimer les fichiers GGUF du dossier models
    let models_dir = default_models_dir();
    if models_dir.exists() {
        match std::fs::remove_dir_all(&models_dir) {
            Ok(_) => {
                // Recréer le dossier vide
                std::fs::create_dir_all(&models_dir)
                    .map_err(|e| format!("Erreur recréation dossier models: {e}"))?;
            }
            Err(e) => return Err(format!("Erreur suppression dossier models: {e}"))
        }
    }
    
    Ok(())
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
        model_type: "general".to_string(),
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
pub fn hardware_get() -> Result<HardwareInfo, String> {
    Ok(hardware::detect())
}

#[tauri::command]
pub fn hardware_can_run_model(ram_min_gb: u32) -> Result<bool, String> {
    Ok(hardware::detect().can_run_model(ram_min_gb))
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

/// Endpoint Ollama par défaut (instance locale).
const DEFAULT_OLLAMA_ENDPOINT: &str = "http://localhost:11434";

/// Information sur un modèle Ollama installé localement.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: u64,
    pub digest: String,
    pub modified_at: String,
}

/// Appel bas-niveau : récupère la liste des modèles Ollama installés
/// via `GET /api/tags`. Utilisé par la commande IPC et par `list_catalog`.
pub async fn fetch_ollama_models(endpoint: &str) -> Result<Vec<OllamaModelInfo>, String> {
    let url = format!("{}/api/tags", endpoint.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Ollama injoignable sur {endpoint}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Ollama a répondu {}", resp.status()));
    }

    // Structure interne : Ollama renvoie les champs en snake_case.
    #[derive(serde::Deserialize)]
    struct OllamaTag {
        name: String,
        #[serde(default)]
        size: u64,
        #[serde(default)]
        digest: String,
        #[serde(default)]
        modified_at: String,
    }
    #[derive(serde::Deserialize)]
    struct TagsResponse {
        #[serde(default)]
        models: Vec<OllamaTag>,
    }

    let parsed: TagsResponse = resp
        .json()
        .await
        .map_err(|e| format!("réponse Ollama invalide: {e}"))?;
    Ok(parsed
        .models
        .into_iter()
        .map(|t| OllamaModelInfo {
            name: t.name,
            size: t.size,
            digest: t.digest,
            modified_at: t.modified_at,
        })
        .collect())
}

/// Liste les modèles installés localement côté Ollama (`GET /api/tags`).
/// Renvoie une erreur si Ollama n'est pas joignable.
#[tauri::command]
pub async fn ollama_list_models(
    state: State<'_, AppState>,
) -> Result<Vec<OllamaModelInfo>, String> {
    let endpoint = state
        .config
        .get()
        .await
        .provider
        .ollama_endpoint
        .unwrap_or_else(|| DEFAULT_OLLAMA_ENDPOINT.to_string());
    fetch_ollama_models(&endpoint).await
}

/// Lance le pull d'un modèle Ollama (`POST /api/pull`). Le pull est
/// async et émet des events `ollama:pull:progress` / `done` / `error`.
/// Retourne immédiatement après avoir lancé le tokio::task.
#[tauri::command]
pub async fn ollama_pull_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    model: String,
) -> Result<(), String> {
    let endpoint = state
        .config
        .get()
        .await
        .provider
        .ollama_endpoint
        .unwrap_or_else(|| DEFAULT_OLLAMA_ENDPOINT.to_string());
    let endpoint = endpoint.trim_end_matches('/').to_string();
    let model_clone = model.clone();

    // Le pull peut être long : on le lance dans une task et on émet des
    // events de progression. La commande retourne immédiatement.
    tokio::spawn(async move {
        let url = format!("{}/api/pull", endpoint);
        let body = serde_json::json!({ "model": model_clone, "stream": true });

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600)) // pull peut être long
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit(
                    "ollama:pull:error",
                    serde_json::json!({ "model": model_clone, "error": e.to_string() }),
                );
                return;
            }
        };

        let resp = match client.post(&url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = app.emit(
                    "ollama:pull:error",
                    serde_json::json!({
                        "model": model_clone,
                        "error": format!("Ollama injoignable: {e}"),
                    }),
                );
                return;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let _ = app.emit(
                "ollama:pull:error",
                serde_json::json!({
                    "model": model_clone,
                    "error": format!("Ollama a répondu {status}: {text}"),
                }),
            );
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
                        if line_str.is_empty() {
                            continue;
                        }
                        let Ok(parsed) =
                            serde_json::from_str::<serde_json::Value>(line_str)
                        else {
                            continue;
                        };

                        let status = parsed
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        // Fin du pull : Ollama renvoie {"status":"success"}.
                        if status == "success" {
                            let _ = app.emit(
                                "ollama:pull:done",
                                serde_json::json!({ "model": model_clone }),
                            );
                            return;
                        }

                        // Une erreur peut apparaître dans le flux.
                        if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
                            let _ = app.emit(
                                "ollama:pull:error",
                                serde_json::json!({
                                    "model": model_clone,
                                    "error": err,
                                }),
                            );
                            return;
                        }

                        // Throttle : un event de progression toutes les 200ms max.
                        let now = std::time::Instant::now();
                        if now.duration_since(last_emit)
                            >= std::time::Duration::from_millis(200)
                        {
                            last_emit = now;
                            let _ = app.emit("ollama:pull:progress", &parsed);
                        }
                    }
                }
                Err(e) => {
                    let _ = app.emit(
                        "ollama:pull:error",
                        serde_json::json!({
                            "model": model_clone,
                            "error": format!("Flux interrompu: {e}"),
                        }),
                    );
                    return;
                }
            }
        }
    });

    Ok(())
}

// ===== Config par projet =====

/// Retourne la config globale courante (sérialisée en JSON pour le frontend).
#[tauri::command]
pub async fn config_get(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.get().await;
    Ok(config_to_json(&cfg))
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
    Ok(config_to_json(&merged))
}

fn config_to_json(cfg: &crate::config::Config) -> serde_json::Value {
    serde_json::json!({
        "storage": { "modelsDir": cfg.storage.models_dir.to_string_lossy() },
        "permissions": { "overrides": cfg.permissions.overrides },
        "provider": {
            "defaultProvider": cfg.provider.default_provider,
            "defaultModel": cfg.provider.default_model,
            "ollamaEndpoint": cfg.provider.ollama_endpoint,
        }
    })
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
#[serde(rename_all = "camelCase")]
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
