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
//!
//! En J2 les sessions et leurs messages sont persistés en SQLite
//! (`~/.cyonima/sessions.db`) via [`persistence::Persistence`]. Le
//! SessionManager garde un cache in-memory (DashMap) pour pouvoir
//! streamer/spawner des tokio::tasks, mais l'état vérité vie en DB :
//! au démarrage de l'app on charge les sessions historiques et leurs messages,
//! et chaque mutation (create/send/fork/assistant/tool) est flushée.

pub mod agents_md;
pub mod persistence;

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
#[serde(rename_all = "camelCase")]
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
    /// Modèle courant, sélectionné via le menu déroulant du chat. Peut être
    /// vide à la création : l'UI le renseigne avant le premier envoi.
    pub current_model: Mutex<String>,
    /// Intensité de raisonnement courante ("auto"/"off"/"low"/"medium"/"high").
    pub current_reasoning: Mutex<String>,
    /// Permet à `session_cancel` d'interrompre le stream courant.
    pub cancel: CancellationToken,
    /// `true` si un stream est en cours — protège contre les envois concurrents.
    pub busy: Mutex<bool>,
    /// Snapshot optionnel de la persistence pour flusher les messages
    /// au fil de l'eau (None pour les tests unitaires sans DB).
    persistence: Option<persistence::Persistence>,
}

pub struct SessionManager {
    sessions: DashMap<String, Arc<SessionInner>>,
    persistence: Option<persistence::Persistence>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self {
            sessions: DashMap::new(),
            persistence: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenEvent {
    pub session_id: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallEvent {
    pub session_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultEvent {
    pub session_id: String,
    pub call_id: String,
    pub tool: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingEvent {
    pub session_id: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLoadingEvent {
    pub session_id: String,
    pub loading: bool,
    pub progress: f32, // 0.0 - 100.0
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoneEvent {
    pub session_id: String,
    pub usage: providers::Usage,
    /// Message assistant complet dernier de la boucle (sans les tool calls
    /// intermédiaires — pour persistance future, J2).
    pub assistant_message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEvent {
    pub session_id: String,
    pub error: String,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construit avec une persistence déjà ouverte. Au démarrage de l'app,
    /// on charge immédiatement les sessions historiques via [`restore_all`].
    pub fn with_persistence(p: persistence::Persistence) -> Self {
        Self {
            sessions: DashMap::new(),
            persistence: Some(p),
        }
    }

    /// Au démarrage : charge toutes les sessions persistées + leurs messages
    /// dans le cache in-memory. On recrée le provider/registry, et on préfixe
    /// les messages AGENTS.md если disponible.
    pub async fn restore_all(&self) -> anyhow::Result<()> {
        let Some(p) = &self.persistence else {
            return Ok(());
        };
        let sessions = p.list_sessions().await?;
        for info in sessions {
            let provider = providers::build(providers::ProviderParams {
                kind: info.provider_id,
                endpoint: None,
                api_key: None,
            });
            let abs_workspace = canonicalize_workspace(&info.workspace);
            let mut messages: Vec<ChatMessage> = Vec::new();
            // AGENTS.md en tête (reloadé — il peut avoir changé entre temps).
            if let Some(agents_content) = agents_md::load(&abs_workspace) {
                messages.push(ChatMessage {
                    role: Role::System,
                    content: agents_content,
                });
            }
            // Suit les messages persisté (user/assistant/tool — pas l'AGENTS.md).
            let persisted = p.load_messages(&info.id).await?;
            messages.extend(persisted);
            let inner = Arc::new(SessionInner {
                info: info.clone(),
                provider,
                tools: ToolRegistry::built_in(),
                messages: Mutex::new(messages),
                current_model: Mutex::new(info.model_id.clone()),
                current_reasoning: Mutex::new("auto".to_string()),
                cancel: CancellationToken::new(),
                busy: Mutex::new(false),
                persistence: self.persistence.clone(),
            });
            self.sessions.insert(info.id.clone(), inner);
        }
        Ok(())
    }

    /// Crée une session, instancie le provider + le registry + le AGENTS.md.
    /// Persiste la session en SQLite si une persistence est attachée.
    pub async fn create(
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

        // AGENTS.md injecté comme system prompt s'il existe. NON persisté
        // en DB : il est rechargé à chaque `restore_all`, ce qui permet de
        // le modifier sans casser l'historique.
        if let Some(agents_content) = agents_md::load(&abs_workspace) {
            initial_messages.push(ChatMessage {
                role: Role::System,
                content: agents_content,
            });
        }

        if let Some(p) = &self.persistence {
            // On ne peut propager l'erreur sans casser l'API sync IPC. On log
            // et on continue : une session non persistée reste fonctionnelle
            // en mémoire (elle sera juste perdue au redémarrage).
            if let Err(e) = p.upsert_session(&info).await {
                tracing::warn!("échec persistance session {}: {e}", info.id);
            }
        }

        let inner = Arc::new(SessionInner {
            info: info.clone(),
            provider,
            tools: ToolRegistry::built_in(),
            messages: Mutex::new(initial_messages),
            current_model: Mutex::new(info.model_id.clone()),
            current_reasoning: Mutex::new("auto".to_string()),
            cancel: CancellationToken::new(),
            busy: Mutex::new(false),
            persistence: self.persistence.clone(),
        });
        self.sessions.insert(info.id.clone(), inner);
        let _ = gateway; // état partagé via AppState
        info
    }

    /// Liste les sessions — lit depuis le cache in-memory (qui est toujours
    /// synchronisé avec la DB, car `restore_all` est appelée au démarrage).
    pub fn list(&self) -> Vec<SessionInfo> {
        self.sessions.iter().map(|e| e.info.clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<Arc<SessionInner>> {
        self.sessions.get(id).map(|r| Arc::clone(&r))
    }

    /// Récupère l'historique des messages d'une session, sans le AGENTS.md
    /// system (qui est interne au runtime). Exposé à l'UI pour restauration.
    pub async fn history(&self, id: &str) -> Option<Vec<ChatMessage>> {
        let session = self.get(id)?;
        let msgs = session.messages.lock().await.clone();
        // Filtrage du system AGENTS.md (contenu uniquement en mémoire, jamais
        // persisté). Pour distinguer un vrai AGENTS.md d'un system utilisateur
        // futur, on garde ici tous les system. L'UI peut décider de les cacher.
        Some(msgs)
    }

    /// Fork : copie du contexte vers une nouvelle session avec son propre
    /// `CancellationToken`. En J2 **persiste** aussi les messages.
    pub async fn fork(&self, gateway: Arc<Gateway>, id: &str) -> Option<SessionInfo> {
        let src = self.get(id)?;
        let forked = SessionInfo::new(
            src.info.workspace.clone(),
            src.info.model_id.clone(),
            src.info.provider_id,
        );
        let provider = src.provider.clone();
        let tools = src.tools.clone();
        let messages = src.messages.lock().await.clone();

        // Persiste la session fork...
        if let Some(p) = &src.persistence {
            if let Err(e) = p.upsert_session(&forked).await {
                tracing::warn!("échec persistance fork {}: {e}", forked.id);
            }
            // ... + persiste les messages NON system (AGENTS.md n'a pas à être
            // copié dans la DB — il sera rechargé au prochain démarrage).
            for m in &messages {
                if m.role == Role::System {
                    continue;
                }
                if let Err(e) = p.append_message(&forked.id, m).await {
                    tracing::warn!("échec persistance msg fork {}: {e}", forked.id);
                }
            }
        }

        let current_model = { src.current_model.lock().await.clone() };
        let current_reasoning = { src.current_reasoning.lock().await.clone() };
        let inner = Arc::new(SessionInner {
            info: forked.clone(),
            provider,
            tools,
            messages: Mutex::new(messages),
            current_model: Mutex::new(current_model),
            current_reasoning: Mutex::new(current_reasoning),
            cancel: CancellationToken::new(),
            busy: Mutex::new(false),
            persistence: src.persistence.clone(),
        });
        self.sessions.insert(forked.id.clone(), inner);
        let _ = gateway;
        Some(forked)
    }

    /// Envoie un message utilisateur et lance la boucle d'agent.
    ///
    /// `model` : modèle sélectionné dans le menu déroulant du chat. S'il est
    /// fourni (et non vide), il devient le modèle courant de la session.
    pub async fn send(
        &self,
        app: AppHandle,
        gateway: Arc<Gateway>,
        session_id: String,
        message: String,
        model: Option<String>,
        reasoning: Option<String>,
    ) -> Result<(), String> {
        tracing::info!("=== DEBUT session_send pour session {} ===", session_id);
        tracing::info!("Message reçu: '{}'", message);

        let session = self
            .get(&session_id)
            .ok_or_else(|| format!("Session '{session_id}' introuvable"))?;

        tracing::info!("Session trouvée, provider: {:?}", session.info.provider_id);

        // Met à jour le modèle courant si l'UI en a fourni un.
        if let Some(m) = model {
            if !m.trim().is_empty() {
                *session.current_model.lock().await = m;
            }
        }
        // Met à jour l'intensité de raisonnement si fournie.
        if let Some(r) = reasoning {
            if !r.trim().is_empty() {
                *session.current_reasoning.lock().await = r;
            }
        }

        {
            let mut busy = session.busy.lock().await;
            if *busy {
                tracing::warn!("Session {} déjà en cours", session_id);
                return Err(
                    "Un stream est déjà en cours sur cette session — annulez-le d'abord.".into(),
                );
            }
            *busy = true;
        }

        let user_msg = ChatMessage {
            role: Role::User,
            content: message.clone(),
        };
        {
            let mut msgs = session.messages.lock().await;
            msgs.push(user_msg.clone());
            tracing::info!("Message utilisateur ajouté, total messages: {}", msgs.len());
        }
        if let Some(p) = &session.persistence {
            if let Err(e) = p.append_message(&session_id, &user_msg).await {
                tracing::warn!("échec persistance user msg {session_id}: {e}");
            }
        }

        let task_session = Arc::clone(&session);
        tracing::info!(
            "Lancement de la tâche agent_loop pour session {}",
            session_id
        );
        tokio::spawn(async move {
            agent_loop(app, gateway, task_session).await;
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

    /// Supprime une session (cache in-memory + SQLite). Cascade supprime
    /// tous ses messages via `ON DELETE CASCADE`.
    pub async fn delete(&self, session_id: &str) -> Result<(), String> {
        self.sessions.remove(session_id);
        if let Some(p) = &self.persistence {
            p.delete_session(session_id)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Boucle d'agent : LLM stream → tool calls → permission → exec → re-LLM.
async fn agent_loop(app: AppHandle, gateway: Arc<Gateway>, session: Arc<SessionInner>) {
    let workspace = canonicalize_workspace(&session.info.workspace);
    let cancel = session.cancel.clone();
    let session_id = session.info.id.clone();
    let specs = session.tools.specs();
    let model = { session.current_model.lock().await.clone() };
    let reasoning = { session.current_reasoning.lock().await.clone() };

    tracing::info!("=== DEBUT agent_loop pour session {} ===", session_id);
    tracing::info!("Provider: {:?}, Model: {}", session.info.provider_id, model);

    // Sans modèle sélectionné, on ne peut rien envoyer au provider.
    if model.trim().is_empty() {
        let _ = app.emit(
            "session:error",
            ErrorEvent {
                session_id: session_id.clone(),
                error:
                    "Aucun modèle sélectionné — choisissez-en un dans le menu déroulant du chat."
                        .into(),
            },
        );
        let mut busy = session.busy.lock().await;
        *busy = false;
        return;
    }

    let mut final_assistant = String::new();
    let mut final_usage = providers::Usage::default();
    let mut errored = false;

    for iteration in 0..=MAX_TOOL_ITERATIONS {
        if cancel.is_cancelled() {
            tracing::info!("Agent loop annulée pour session {}", session_id);
            break;
        }

        tracing::info!("Itération {} pour session {}", iteration, session_id);

        let messages = { session.messages.lock().await.clone() };
        tracing::info!("Nombre de messages dans l'historique: {}", messages.len());

        let req = ChatRequest {
            messages,
            model: model.clone(),
            tools: specs.clone(),
            reasoning: Some(reasoning.clone()),
            ..Default::default()
        };

        tracing::info!("Appel du provider.stream() pour session {}", session_id);

        // Pour LlamaCpp, émettre un événement de début de chargement si c'est le premier appel
        if session.info.provider_id == crate::providers::ProviderKind::LlamaCpp {
            let _ = app.emit(
                "session:model_loading",
                crate::sessions::ModelLoadingEvent {
                    session_id: session_id.clone(),
                    loading: true,
                    progress: 10.0, // Progression indéterminée au début
                },
            );
        }

        let mut stream = session.provider.stream(req).await;
        let mut assistant_buffer = String::new();
        let mut tool_calls: Vec<providers::ToolCall> = Vec::new();
        let mut had_error = false;
        let mut first_token = true;

        tracing::info!("Début de lecture du stream pour session {}", session_id);
        while let Some(event) = stream.next().await {
            if cancel.is_cancelled() {
                tracing::info!("Stream annulé pour session {}", session_id);
                break;
            }
            match event {
                ChatEvent::Token(tok) => {
                    // Émettre la fin du chargement au premier token pour LlamaCpp
                    if first_token
                        && session.info.provider_id == crate::providers::ProviderKind::LlamaCpp
                    {
                        let _ = app.emit(
                            "session:model_loading",
                            crate::sessions::ModelLoadingEvent {
                                session_id: session_id.clone(),
                                loading: false,
                                progress: 100.0,
                            },
                        );
                        first_token = false;
                    }

                    tracing::debug!("Token reçu pour session {}: '{}'", session_id, tok);
                    assistant_buffer.push_str(&tok);
                    let _ = app.emit(
                        "session:token",
                        TokenEvent {
                            session_id: session_id.clone(),
                            token: tok,
                        },
                    );
                }
                ChatEvent::Thinking(tok) => {
                    tracing::debug!("Thinking reçu pour session {}: '{}'", session_id, tok);
                    let _ = app.emit(
                        "session:thinking",
                        ThinkingEvent {
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
                    tracing::error!("Erreur dans le stream pour session {}: {}", session_id, err);

                    // Émettre la fin du chargement en cas d'erreur pour LlamaCpp
                    if session.info.provider_id == crate::providers::ProviderKind::LlamaCpp {
                        let _ = app.emit(
                            "session:model_loading",
                            crate::sessions::ModelLoadingEvent {
                                session_id: session_id.clone(),
                                loading: false,
                                progress: 0.0, // 0 pour indiquer une erreur
                            },
                        );
                    }

                    had_error = true;
                    errored = true;
                    let _ = app.emit(
                        "session:error",
                        ErrorEvent {
                            session_id: session_id.clone(),
                            error: err.clone(),
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
            let assistant_msg = ChatMessage {
                role: Role::Assistant,
                content: assistant_buffer.clone(),
            };
            session.messages.lock().await.push(assistant_msg.clone());
            final_assistant = assistant_buffer.clone();
            if let Some(p) = &session.persistence {
                if let Err(e) = p.append_message(&session_id, &assistant_msg).await {
                    tracing::warn!("échec persistance assistant msg {session_id}: {e}");
                }
            }
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
                let tool_msg = ChatMessage {
                    role: Role::Tool,
                    content: format!("Outil `{}` refusé par l'utilisateur.", tc.tool),
                };
                session.messages.lock().await.push(tool_msg.clone());
                if let Some(p) = &session.persistence {
                    if let Err(e) = p.append_message(&session_id, &tool_msg).await {
                        tracing::warn!("échec persistance tool(denied) msg {session_id}: {e}");
                    }
                }
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
            let tool_msg = ChatMessage {
                role: Role::Tool,
                content: output.output.clone(),
            };
            session.messages.lock().await.push(tool_msg.clone());
            if let Some(p) = &session.persistence {
                if let Err(e) = p.append_message(&session_id, &tool_msg).await {
                    tracing::warn!("échec persistance tool msg {session_id}: {e}");
                }
            }
        }

        // La boucle remonte et renvoie le contexte mis à jour au LLM.
        let _ = denied_any;
    }

    {
        let mut busy = session.busy.lock().await;
        *busy = false;
        tracing::info!("Session {} libérée (busy=false)", session_id);
    }

    if !errored && !cancel.is_cancelled() {
        tracing::info!("Émission de session:done pour session {}", session_id);
        let _ = app.emit(
            "session:done",
            DoneEvent {
                session_id: session_id.clone(),
                usage: final_usage,
                assistant_message: final_assistant,
            },
        );
    } else {
        tracing::warn!(
            "Agent loop terminé avec erreur ou annulation pour session {}",
            session_id
        );
    }
    tracing::info!("=== FIN agent_loop pour session {} ===", session_id);
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
