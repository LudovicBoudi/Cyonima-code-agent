//! Provider Ollama — HTTP streaming vers une instance Ollama locale externe.
//!
//! Compatible avec le tool-use natif Ollama : on envoie `tools` dans le body,
//! on parse `tool_calls` des chunks NDJSON et on les émet comme
//! `ChatEvent::ToolCall`. Le `SessionManager` consomme ces tool calls, demande
//! les permissions, exécute, puis renvoie un message `tool` au LLM pour
//! continuer la conversation.

use std::time::Duration;

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::{Capabilities, ChatEvent, ChatRequest, Provider, Usage};

/// Endpoint par défaut d'Ollama.
pub const DEFAULT_ENDPOINT: &str = "http://localhost:11434";

pub struct OllamaProvider {
    endpoint: String,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: OllamaOptions,
    /// Outils activés (format OpenAI-compatible function calling).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tools: Vec<OllamaTool>,
}

#[derive(Debug, Clone, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
    /// Images embed pour les modèles multimodal (non utilisé en J1).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    images: Vec<String>,
    /// Pour les messages renvoyés par l'agent suite à un tool call.
    /// (format OpenAI-compatible : le tool_call_id est dans `tool_calls`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    tool_call_id: Option<String>,
    /// Outil invoqué par l'assistant — format Ollama.
    /// Présent sur les messages assistant contenant un tool call.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Debug, Clone, Serialize)]
struct OllamaTool {
    #[serde(rename = "type")]
    kind: String,
    function: OllamaToolDef,
}

#[derive(Debug, Clone, Serialize)]
struct OllamaToolDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
struct OllamaToolCall {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Default)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "num_predict")]
    num_predict: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaChatChunk {
    message: Option<OllamaChunkMessage>,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaChunkMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaChunkToolCall>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaChunkToolCall {
    function: OllamaChunkToolFn,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaChunkToolFn {
    name: String,
    arguments: serde_json::Value,
}

impl OllamaProvider {
    pub fn new(endpoint: Option<String>) -> Self {
        let endpoint = endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .expect("échec de construction du client HTTP pour Ollama");
        Self { endpoint, client }
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn id(&self) -> &str {
        "ollama"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            // Ollama supporte les tools nativement sur les modèles compatibles
            // (Llama 3.1+, Mistral, Qwen 2.5, Gemma 4). On déclare `true` et
            // l'utilisateur choisit un modèle qui l'accepte dans l'UI.
            supports_tools: true,
            supports_vision: false,
            context_window: 8192,
        }
    }

    async fn stream(&self, req: ChatRequest) -> BoxStream<'static, ChatEvent> {
        let url = format!("{}/api/chat", self.endpoint.trim_end_matches('/'));
        let tools = req
            .tools
            .iter()
            .map(|t| OllamaTool {
                kind: "function".into(),
                function: OllamaToolDef {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect::<Vec<_>>();

        let body = OllamaChatRequest {
            model: req.model.clone(),
            messages: req
                .messages
                .iter()
                .map(|m| OllamaMessage {
                    role: m.role.as_str().into(),
                    content: m.content.clone(),
                    images: Vec::new(),
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                })
                .collect(),
            stream: true,
            options: OllamaOptions {
                temperature: req.temperature,
                num_predict: req.max_tokens,
            },
            tools,
        };

        let client = self.client.clone();

        let s = async_stream::stream! {
            let response = match client.post(&url).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    yield ChatEvent::Error(format!(
                        "Ollama injoignable sur {url} — vérifiez qu'Ollama tourne (ollama serve). Détail: {e}"
                    ));
                    yield ChatEvent::Done(Usage::default());
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                yield ChatEvent::Error(format!("Ollama a répondu {status}: {text}"));
                yield ChatEvent::Done(Usage::default());
                return;
            }

            let mut bytes_stream = response.bytes_stream();
            let mut buffer: Vec<u8> = Vec::new();
            let mut usage = Usage::default();

            while let Some(chunk_res) = bytes_stream.next().await {
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
                            let Ok(parsed) = serde_json::from_str::<OllamaChatChunk>(line_str) else { continue };
                            if let Some(msg) = parsed.message {
                                if !msg.content.is_empty() {
                                    yield ChatEvent::Token(msg.content);
                                }
                                for tc in msg.tool_calls {
                                    yield ChatEvent::ToolCall(super::ToolCall {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        tool: tc.function.name,
                                        arguments: tc.function.arguments,
                                    });
                                }
                            }
                            if parsed.done {
                                usage.tokens_in = parsed.prompt_eval_count.unwrap_or(0);
                                usage.tokens_out = parsed.eval_count.unwrap_or(0);
                                yield ChatEvent::Done(usage);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        yield ChatEvent::Error(format!("flux Ollama interrompu: {e}"));
                        yield ChatEvent::Done(usage);
                        return;
                    }
                }
            }
            // Ollama a coupé sans `done: true` (timeout / déconnexion).
            yield ChatEvent::Done(usage);
        };

        s.boxed()
    }
}
