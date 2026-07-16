//! Providers : abstraction multi-backends pour l'inférence IA.
//!
//! Tout backend (local ou distant) implémente le trait [`Provider`]. Les
//! sessions et l'UI sont agnostiques du modèle concret. Cf `docs/ARCHITECTURE.md`.
//!
//! Implémentations :
//! - [`ollama`]      : HTTP streaming vers Ollama local externe (J1, fonctionnel)
//! - [`llama_cpp`]   : bindings built-in via candle GGUF (J1 — stub, à câbler en J1.5)
//! - `openai`        : API distante OpenAI (J6)
//! - `anthropic`     : API distante Anthropic (J6)
//! - `gemini`        : API distante Google Gemini (J6)
//! - `openai_compat` : endpoint OpenAI-compatible type LM Studio / vLLM / entreprise (J6)

pub mod llama_cpp;
pub mod ollama;

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Message de chat élémentaire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

/// Demande de complétion streaming adressée à un provider.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    /// Température (0.0 = greedy par défaut).
    pub temperature: Option<f32>,
    /// Nombre max de tokens à générer.
    pub max_tokens: Option<u32>,
    /// Outils activés pour cette session (noms d'outils).
    pub tools: Vec<String>,
    /// Identifiant du modèle côté provider (ex: "llama3.2" pour Ollama,
    /// chemin GGUF pour llama_cpp). Renseigné par le session manager.
    pub model: String,
}

impl Default for ChatRequest {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            temperature: Some(0.7),
            max_tokens: Some(2048),
            tools: Vec::new(),
            model: String::new(),
        }
    }
}

/// Événement émis en streaming par un provider.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatEvent {
    /// Un token a été généré.
    Token(String),
    /// Le modèle demande l'exécution d'un outil.
    ToolCall(ToolCall),
    /// Le résultat d'un outil a été consommé par le modèle.
    ToolResult(ToolResult),
    /// La génération est terminée.
    Done(Usage),
    /// Une erreur est survenue côté provider.
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCall {
    pub id: String,
    pub tool: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResult {
    pub call_id: String,
    pub output: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Usage {
    pub tokens_in: u32,
    pub tokens_out: u32,
}

/// Capabilities déclarées par un provider, utilisées par l'UI et l'orchestrateur
/// de sessions pour adapter le prompt (vision, taille de contexte, outils, etc.).
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub context_window: u32,
}

/// Abstraction universelle d'un backend d'inférence.
///
/// Toute nouvelle implémentation (locale ou distante) doit résider dans ce
/// module. Pas de logique spécifique à un backend en dehors de `providers/`.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Identifiant stable du provider (ex: `llama_cpp`, `openai`, `ollama`).
    fn id(&self) -> &str;

    /// Liste capabilities statiques disponibles pour ce provider.
    fn capabilities(&self) -> Capabilities;

    /// Lance une complétion streaming. Le stream doit **toujours** se
    /// terminer par un `ChatEvent::Done` ou `ChatEvent::Error`.
    async fn stream(&self, req: ChatRequest) -> BoxStream<'static, ChatEvent>;
}

/// Catalogue des providers connus. Sert de dispatch au session manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    LlamaCpp,
    Ollama,
    OpenAi,
    Anthropic,
    Gemini,
    OpenAiCompat,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LlamaCpp => "llama_cpp",
            Self::Ollama => "ollama",
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::OpenAiCompat => "openai_compat",
        }
    }
}

/// Paramètres construction d'un provider. Permet à [`build`] de router vers
/// la bonne impl sans exposer les détails au session manager.
#[derive(Debug, Clone)]
pub struct ProviderParams {
    pub kind: ProviderKind,
    /// Endpoint HTTP pour les providers réseau (Ollama par défaut = http://localhost:11434).
    pub endpoint: Option<String>,
    /// Clé API (pour J6 — providers distants).
    pub api_key: Option<String>,
}

/// Factory de providers. Le session manager appelle [`build`] quand il crée
/// une session ; l'instance produite vit le temps de la session.
pub fn build(params: ProviderParams) -> Arc<dyn Provider> {
    match params.kind {
        ProviderKind::Ollama => Arc::new(ollama::OllamaProvider::new(params.endpoint)),
        ProviderKind::LlamaCpp => Arc::new(llama_cpp::LlamaCppProvider::new()),
        // J6 : implémentations distantes. Pour J1, on retombe sur une erreur
        // claire émise au premier appel.
        other => Arc::new(UnimplementedProvider { id: other.as_str() }),
    }
}

struct UnimplementedProvider {
    id: &'static str,
}

#[async_trait]
impl Provider for UnimplementedProvider {
    fn id(&self) -> &str {
        self.id
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    async fn stream(&self, _req: ChatRequest) -> BoxStream<'static, ChatEvent> {
        let msg = format!("Provider '{}' non implémenté (prévu en J6+).", self.id);
        futures::stream::once(async move { ChatEvent::Error(msg) }).boxed()
    }
}
