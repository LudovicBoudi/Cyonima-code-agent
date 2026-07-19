//! Gestionnaire de sessions — multi-session parallèle sur le même projet.
//!
//! En J3 la session devient un **agent** : la réponse streamée peut contenir
//! des tool_calls, qui sont routés via la gateway de permissions, exécutés via
//! le `ToolRegistry`, puis le résultat est réinjecté au LLM pour continuer
//! la conversation. La boucle est bornée par `MAX_TOOL_ITERATIONS` (32) pour
//! éviter une dérive infinie même si un modèle refuse de s'arrêter.
//!
//! AGENTS.md (s'il existe à la racine du workspace) est injecté comme premier
//! message `system` du contexte, conformément à la convention Opencode.

pub mod agents_md;

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::permissions::{Decision, Gateway};
use crate::providers::{self, ChatEvent, ChatMessage, ChatRequest, Provider, ProviderKind, Role};
use crate::tools::ToolRegistry;

/// Nombre maximum d'itérations tool-call → tool-result par envoi utilisateur.
/// Au-delà, on coupe et on termine la session avec un message d'erreur.
const MAX_TOOL_ITERATIONS: usize = 32;

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

pub struct SessionInner {
    pub info: SessionInfo,
    pub provider: Arc<dyn Provider>,
    pub tools: ToolRegistry,
    /// Contexte de conversation (incluant le AGENTS.md en tête). Mutex car
    /// partagé entre la task de streaming et les forks.
    pub messages: Mutex<Vec<ChatMessage>>,
    /// Permet à `session_cancel` d'interrompre le stream courant.
    pub cancel: CancellationToken,
    /// `true` si un stream est en cours — protège contre les envois concurrents.
    pub busy: Mutex<bool>,
}

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
pub struct ToolCallEvent {
    pub session_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResultEvent {
    pub session_id: String,
    pub call_id: String,
    pub tool: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoneEvent {
    pub session_id: String,
    pub usage: providers::Usage,
    /// Message assistant complet dernier de la boucle (sans les tool calls
    /// intermédiaires — pour persistance future, J2).
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

    /// Crée une session, instancie le provider + le registry + le AGENTS.md.
    pub fn create(
        &self,
        gateway: Arc<Gateway>,
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
        let info = SessionInfo::new(workspace.clone(), model_id, provider_kind);
        let abs_workspace = canonicalize_workspace(&workspace);
        let mut initial_messages: Vec<ChatMessage> = Vec::new();

        // AGENTS.md injecté comme system prompt s'il existe.
        if let Some(agents_content) = agents_md::load(&abs_workspace) {
            initial_messages.push(ChatMessage {
                role: Role::System,
                content: agents_content,
            });
        }

        let inner = Arc::new(SessionInner {
            info: info.clone(),
            provider,
            tools: ToolRegistry::built_in(),
            messages: Mutex::new(initial_messages),
            cancel: CancellationToken::new(),
            busy: Mutex::new(false),
        });
        self.sessions.insert(info.id.clone(), inner);
        let _ = gateway; // état partagé via AppState
        info
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        self.sessions.iter().map(|e| e.info.clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<Arc<SessionInner>> {
        self.sessions.get(id).map(|r| Arc::clone(&r))
    }

    /// Fork : copie du contexte vers une nouvelle session avec son propre
    /// `CancellationToken`. Implémentation V1 : copie in-memory. J2 = persistance.
    pub fn fork(&self, gateway: Arc<Gateway>, id: &str) -> Option<SessionInfo> {
        let src = self.get(id)?;
        let forked = SessionInfo::new(
            src.info.workspace.clone(),
            src.info.model_id.clone(),
            src.info.provider_id,
        );
        let provider = src.provider.clone();
        let tools = src.tools.clone();
        let messages = src.messages.blocking_lock().clone();
        let inner = Arc::new(SessionInner {
            info: forked.clone(),
            provider,
            tools,
            messages: Mutex::new(messages),
            cancel: CancellationToken::new(),
            busy: Mutex::new(false),
        });
        self.sessions.insert(forked.id.clone(), inner);
        let _ = gateway;
        Some(forked)
    }

    /// Envoie un message utilisateur et lance la boucle d'agent.
    pub fn send(
        &self,
        app: AppHandle,
        gateway: Arc<Gateway>,
        session_id: String,
        message: String,
    ) -> Result<(), String> {
        let session = self
            .get(&session_id)
            .ok_or_else(|| format!("Session '{session_id}' introuvable"))?;

        {
            let mut busy = tokio::task::block_in_place(|| session.busy.blocking_lock());
            if *busy {
                return Err(
                    "Un stream est déjà en cours sur cette session — annulez-le d'abord.".into(),
                );
            }
            *busy = true;
        }

        {
            let mut msgs = tokio::task::block_in_place(|| session.messages.blocking_lock());
            msgs.push(ChatMessage {
                role: Role::User,
                content: message,
            });
        }

        let task_session = Arc::clone(&session);
        tokio::spawn(async move {
            agent_loop(app, gateway, task_session).await;
        });
        Ok(())
    }

    pub fn cancel(&self, session_id: &str) -> Result<(), String> {
        let session = self
            .get(session_id)
            .ok_or_else(|| format!("Session '{session_id}' introuvable"))?;
        session.cancel.cancel();
        Ok(())
    }
}

/// Boucle d'agent : LLM stream → tool calls → permission → exec → re-LLM.
async fn agent_loop(app: AppHandle, gateway: Arc<Gateway>, session: Arc<SessionInner>) {
    let workspace = canonicalize_workspace(&session.info.workspace);
    let cancel = session.cancel.clone();
    let session_id = session.info.id.clone();
    let specs = session.tools.specs();

    let mut final_assistant = String::new();
    let mut final_usage = providers::Usage::default();
    let mut errored = false;

    for _ in 0..=MAX_TOOL_ITERATIONS {
        if cancel.is_cancelled() {
            break;
        }

        let messages = { session.messages.lock().await.clone() };
        let req = ChatRequest {
            messages,
            model: session.info.model_id.clone(),
            tools: specs.clone(),
            ..Default::default()
        };

        let mut stream = session.provider.stream(req).await;
        let mut assistant_buffer = String::new();
        let mut tool_calls: Vec<providers::ToolCall> = Vec::new();
        let mut had_error = false;

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
                ChatEvent::ToolCall(tc) => {
                    tool_calls.push(tc.clone());
                    let _ = app.emit(
                        "session:tool_call",
                        ToolCallEvent {
                            session_id: session_id.clone(),
                            call_id: tc.id.clone(),
                            tool: tc.tool.clone(),
                            arguments: tc.arguments.clone(),
                        },
                    );
                }
                ChatEvent::Done(usage) => {
                    final_usage = usage;
                    break;
                }
                ChatEvent::Error(err) => {
                    had_error = true;
                    errored = true;
                    let _ = app.emit(
                        "session:error",
                        ErrorEvent {
                            session_id: session_id.clone(),
                            error: err,
                        },
                    );
                }
                ChatEvent::ToolResult(_) => {
                    // émis par le manager lui-même, jamais reçu du provider
                }
            }
        }

        // Persiste le message assistant (même si tool-call-only, certains LLM
        // mettent du contenu texte dans le même message).
        if !assistant_buffer.is_empty() {
            session.messages.lock().await.push(ChatMessage {
                role: Role::Assistant,
                content: assistant_buffer.clone(),
            });
            final_assistant = assistant_buffer.clone();
        }

        if had_error || tool_calls.is_empty() || cancel.is_cancelled() {
            break;
        }

        // Exécute séquentiellement les tool calls (V1, pas de parallélisme).
        // Pour chaque : demande permission → exécute → émet event →
        // ajoute message `tool` au contexte.
        let mut denied_any = false;
        for tc in tool_calls {
            if cancel.is_cancelled() {
                break;
            }
            // Demande permission.
            let decision = gateway
                .request(
                    app.clone(),
                    session_id.clone(),
                    &tc.tool,
                    tc.arguments.clone(),
                )
                .await;
            if decision == Decision::Deny {
                denied_any = true;
                let _ = app.emit(
                    "session:tool_result",
                    ToolResultEvent {
                        session_id: session_id.clone(),
                        call_id: tc.id.clone(),
                        tool: tc.tool.clone(),
                        output: "Refusé par l'utilisateur".into(),
                        is_error: true,
                    },
                );
                // On injecte un message `tool` décrivant le refus pour que le
                // LLM sache qu'il doit chercher une alternative.
                session.messages.lock().await.push(ChatMessage {
                    role: Role::Tool,
                    content: format!("Outil `{}` refusé par l'utilisateur.", tc.tool),
                });
                continue;
            }

            // Exécution. Si args invalide, ToolOutput.is_error=true.
            let output = session
                .tools
                .execute(&tc.tool, tc.arguments.clone(), &workspace)
                .await
                .unwrap_or_else(|| crate::tools::ToolOutput::err(&tc.tool, "outil inconnu"));

            let _ = app.emit(
                "session:tool_result",
                ToolResultEvent {
                    session_id: session_id.clone(),
                    call_id: tc.id.clone(),
                    tool: output.tool.clone(),
                    output: output.output.clone(),
                    is_error: output.is_error,
                },
            );
            // Format du message `tool` attendu par Ollama/OpenAI-compatible :
            // le contenu porte le résultat texte rendu au modèle.
            session.messages.lock().await.push(ChatMessage {
                role: Role::Tool,
                content: output.output.clone(),
            });
        }

        // La boucle remonte et renvoie le contexte mis à jour au LLM.
        let _ = denied_any;
    }

    {
        let mut busy = session.busy.lock().await;
        *busy = false;
    }

    if !errored && !cancel.is_cancelled() {
        let _ = app.emit(
            "session:done",
            DoneEvent {
                session_id: session_id.clone(),
                usage: final_usage,
                assistant_message: final_assistant,
            },
        );
    }
}

/// Canonise le chemin workspace en absolu. En cas d'échec (workspace relatif
/// "." par exemple), on retombe sur le CWD si possible, sinon sur le chemin
/// tel quel.
fn canonicalize_workspace(workspace: &str) -> PathBuf {
    let p = PathBuf::from(workspace);
    match p.canonicalize() {
        Ok(c) => c,
        Err(_) => std::env::current_dir().unwrap_or_else(|_| p.clone()),
    }
}
