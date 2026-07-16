//! Provider Ollama — HTTP streaming vers une instance Ollama locale externe.
//!
//! Totalement indépendant des binaires Cyonima : l'utilisateur doit avoir
//! installé Ollama séparément (https://ollama.com) ET tiré un modèle
//! (`ollama pull llama3.2`). Cyonima se contente de parler à son API HTTP
//! de chat en streaming NDJSON.
//!
//! C'est le backend de référence de J1 : testable immédiatement sans
//! embarquer de runtime d'inférence lourd. `LlamaCppProvider` (candle, built-in)
//! arrivera en J1.5 pour le mode 100% local sans dépendance externe.

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
}

#[derive(Debug, Clone, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
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
    content: String,
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
            supports_tools: false, // activable plus tard via format "tools"
            supports_vision: false,
            context_window: 8192, // dépend du modèle, on garde une valeur prudente
        }
    }

    async fn stream(&self, req: ChatRequest) -> BoxStream<'static, ChatEvent> {
        let url = format!("{}/api/chat", self.endpoint.trim_end_matches('/'));
        let body = OllamaChatRequest {
            model: req.model.clone(),
            messages: req
                .messages
                .iter()
                .map(|m| OllamaMessage {
                    role: m.role.as_str().into(),
                    content: m.content.clone(),
                })
                .collect(),
            stream: true,
            options: OllamaOptions {
                temperature: req.temperature,
                num_predict: req.max_tokens,
            },
        };

        let client = self.client.clone();

        // L'appel HTTP et le parsing NDJSON vivent dans un seul `async_stream!`
        // pour retourner directement un `BoxStream`. Les branches d'erreur
        // yield un `ChatEvent::Error` puis un `ChatEvent::Done` puis terminent.
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
                        // Une ligne NDJSON complète = un objet JSON indépendant.
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
