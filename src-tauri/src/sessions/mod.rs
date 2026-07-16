//! Gestionnaire de sessions — multi-session parallèle sur le même projet.
//!
//! Chaque [`SessionInner`] est un agent isolé : son propre provider (instance
//! concrète du trait [`Provider`]), son propre contexte de messages et son
//! propre `CancellationToken`. Le streaming se fait via les events Tauri
//! `session:token` / `session:done` / `session:error` émis depuis un
//! `tokio::task` dédié par envoi.
//!
//! J0/J1 : pas encore de persistance SQLite (prévue en J2). L'état vit en
//! mémoire dans un `DashMap` partagé via `Arc<SessionManager>`.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::providers::{self, ChatEvent, ChatMessage, ChatRequest, Provider, ProviderKind};

/// Métadonnées d'une session, exposées à l'UI via IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    /// Chemin absolu du workspace ouvert.
    pub workspace: String,
    /// Identifiant du modèle côté provider (ex: "llama3.2" pour Ollama,
    /// chemin GGUF pour llama_cpp).
    pub model_id: String,
    /// Backend utilisé pour ce modèle.
    pub provider_id: ProviderKind,
    pub created_at: DateTime<Utc>,
}

impl SessionInfo {
    pub fn new(
        workspace: impl Into<String>,
        model_id: impl Into<String>,
        provider_id: ProviderKind,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            workspace: workspace.into(),
            model_id: model_id.into(),
            provider_id,
            created_at: Utc::now(),
        }
    }
}

/// État complet d'une session (contexte + runtime). Jamais sérialisé tel quel
/// dans l'UI — seules les `SessionInfo` + les messages le sont, via events.
pub struct SessionInner {
    pub info: SessionInfo,
    pub provider: Arc<dyn Provider>,
    /// Contexte de conversation accumulé au fil des échanges. Mutex car
    /// partagé entre la tâche de streaming et d'éventuels forks (J2).
    pub messages: Mutex<Vec<ChatMessage>>,
    /// Permet à `session_cancel` d'interrompre le stream courant.
    pub cancel: CancellationToken,
    /// `true` si un stream est en cours — protège contre les envois concurrents.
    pub busy: Mutex<bool>,
}

/// Le gestionnaire de sessions. Partagé globalement via `Arc` dans le state Tauri.
#[derive(Default)]
pub struct SessionManager {
    sessions: DashMap<String, Arc<SessionInner>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenEvent {
    pub session_id: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoneEvent {
    pub session_id: String,
    pub usage: providers::Usage,
    /// Message assistant complet (utile pour persistance future, J2).
    pub assistant_message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorEvent {
    pub session_id: String,
    pub error: String,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    /// Crée une session, instancie le provider associé et l'enregistre.
    pub fn create(
        &self,
        workspace: String,
        model_id: String,
        provider_kind: ProviderKind,
        endpoint: Option<String>,
    ) -> SessionInfo {
        let provider = providers::build(providers::ProviderParams {
            kind: provider_kind,
            endpoint,
            api_key: None,
        });
        let info = SessionInfo::new(workspace, model_id, provider_kind);
        let inner = Arc::new(SessionInner {
            info: info.clone(),
            provider,
            messages: Mutex::new(Vec::new()),
            cancel: CancellationToken::new(),
            busy: Mutex::new(false),
        });
        self.sessions.insert(info.id.clone(), inner);
        info
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        self.sessions.iter().map(|e| e.info.clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<Arc<SessionInner>> {
        self.sessions.get(id).map(|r| Arc::clone(&r))
    }

    /// Fork en J2 : copie du contexte. Ici on duplique juste la metadata pour
    /// ne pas mentir sur le périmètre J1 (le contexte viendra en J2).
    pub fn fork(&self, id: &str) -> Option<SessionInfo> {
        let src = self.get(id)?;
        let forked = SessionInfo::new(
            src.info.workspace.clone(),
            src.info.model_id.clone(),
            src.info.provider_id,
        );
        let provider = src.provider.clone();
        let messages = src.messages.blocking_lock().clone();
        let inner = Arc::new(SessionInner {
            info: forked.clone(),
            provider,
            messages: Mutex::new(messages),
            cancel: CancellationToken::new(),
            busy: Mutex::new(false),
        });
        self.sessions.insert(forked.id.clone(), inner);
        Some(forked)
    }

    /// Envoie un message utilisateur et lance le streaming de la réponse.
    ///
    /// Le stream tourne dans un `tokio::task` : la commande IPC peut donc
    /// retourner immédiatement, les tokens sont poussés via events Tauri.
    pub fn send(&self, app: AppHandle, session_id: String, message: String) -> Result<(), String> {
        let session = self
            .get(&session_id)
            .ok_or_else(|| format!("Session '{session_id}' introuvable"))?;

        // Refuse un envoi concurrent sur la même session.
        {
            let mut busy = app_handle_block_lock(&session.busy);
            if *busy {
                return Err(
                    "Un stream est déjà en cours sur cette session — annulez-le d'abord.".into(),
                );
            }
            *busy = true;
        }

        // Ajoute immédiatement le message utilisateur au contexte.
        {
            let mut msgs = app_handle_block_lock(&session.messages);
            msgs.push(ChatMessage {
                role: providers::Role::User,
                content: message,
            });
        }

        let task_session = Arc::clone(&session);
        tokio::spawn(async move {
            stream_task(app, task_session).await;
        });
        Ok(())
    }

    /// Annule le stream en cours d'une session (sans détruire la session).
    pub fn cancel(&self, session_id: &str) -> Result<(), String> {
        let session = self
            .get(session_id)
            .ok_or_else(|| format!("Session '{session_id}' introuvable"))?;
        session.cancel.cancel();
        Ok(())
    }

    /// Liste les dernières messages d'une session — utile pour rafraîchir l'UI.
    pub fn history(&self, session_id: &str) -> Option<Vec<ChatMessage>> {
        let session = self.get(session_id)?;
        // block_in_place car appelé depuis une commande IPC sync.
        let msgs = tokio::task::block_in_place(|| session.messages.blocking_lock().clone());
        Some(msgs)
    }
}

/// Boucle de streaming consommée dans un `tokio::task` dédié par `send`.
async fn stream_task(app: AppHandle, session: Arc<SessionInner>) {
    let cancel = session.cancel.clone();
    let session_id = session.info.id.clone();

    // Snapshot du contexte à mettre dans la requête.
    let messages = { session.messages.lock().await.clone() };
    let req = ChatRequest {
        messages,
        model: session.info.model_id.clone(),
        ..Default::default()
    };

    let mut stream = session.provider.stream(req).await;

    let mut assistant_buffer = String::new();
    let mut final_usage = providers::Usage::default();
    let mut errored = false;

    while let Some(event) = stream.next().await {
        if cancel.is_cancelled() {
            break;
        }
        match event {
            ChatEvent::Token(tok) => {
                assistant_buffer.push_str(&tok);
                let _ = app.emit(
                    "session:token",
                    TokenEvent {
                        session_id: session_id.clone(),
                        token: tok,
                    },
                );
            }
            ChatEvent::Done(usage) => {
                final_usage = usage;
                break;
            }
            ChatEvent::Error(err) => {
                errored = true;
                let _ = app.emit(
                    "session:error",
                    ErrorEvent {
                        session_id: session_id.clone(),
                        error: err,
                    },
                );
            }
            ChatEvent::ToolCall(_) | ChatEvent::ToolResult(_) => {
                // Tool-use wiring prévu en J3 — ignoré en J1.
            }
        }
    }

    // Persiste le message assistant dans le contexte (même si erreur partielle).
    if !assistant_buffer.is_empty() {
        let mut msgs = session.messages.lock().await;
        msgs.push(ChatMessage {
            role: providers::Role::Assistant,
            content: assistant_buffer.clone(),
        });
    }

    // Tag la session comme libre pour un prochain envoi.
    {
        let mut busy = session.busy.lock().await;
        *busy = false;
    }

    if !errored {
        let _ = app.emit(
            "session:done",
            DoneEvent {
                session_id: session_id.clone(),
                usage: final_usage,
                assistant_message: assistant_buffer,
            },
        );
    }
}

/// Helper : lock synchrone d'un `tokio::Mutex` depuis une commande IPC qui
/// n'est pas async. On utilise `block_in_place` pour ne pas bloquer le
/// runtime sur une contention éventuelle.
fn app_handle_block_lock<T>(m: &Mutex<T>) -> tokio::sync::MutexGuard<'_, T> {
    tokio::task::block_in_place(|| m.blocking_lock())
}
