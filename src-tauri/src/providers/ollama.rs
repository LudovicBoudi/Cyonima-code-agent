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
    /// Active la séparation du raisonnement dans le champ `thinking`.
    /// Uniquement envoyé pour les modèles qui déclarent la capacité `thinking`
    /// (sinon Ollama renvoie une erreur "does not support thinking").
    #[serde(skip_serializing_if = "Option::is_none")]
    think: Option<bool>,
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
    /// Champ thinking/reasoning présent sur les modèles qui supportent le
    /// reasoning (DeepSeek R1, Gemma 4, Qwen3, etc.). Ollama ≥ 0.7.
    #[serde(default)]
    thinking: String,
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

/// Capacités d'un modèle Ollama, détectées via `/api/show`.
#[derive(Debug, Clone, Copy)]
struct ModelCaps {
    tools: bool,
    thinking: bool,
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

    /// Interroge `POST /api/show` pour connaître les capacités du modèle
    /// (`tools`, `thinking`). Cela évite d'envoyer `tools`/`think` à un modèle
    /// qui ne les supporte pas — ce qui fait échouer la requête avec un HTTP
    /// 400 (ex: DeepSeek-R1 ne supporte pas les tools).
    ///
    /// En cas d'échec (Ollama trop ancien, pas de champ `capabilities`), on
    /// retombe sur `{ tools: true, thinking: false }` : comportement historique
    /// avec retry sans tools si le modèle renvoie une erreur.
    async fn model_capabilities(&self, model: &str) -> ModelCaps {
        let url = format!("{}/api/show", self.endpoint.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "model": model }))
            .send()
            .await;
        let Ok(resp) = resp else {
            return ModelCaps { tools: true, thinking: false };
        };
        if !resp.status().is_success() {
            return ModelCaps { tools: true, thinking: false };
        }

        #[derive(Deserialize)]
        struct ShowResp {
            #[serde(default)]
            capabilities: Vec<String>,
        }
        match resp.json::<ShowResp>().await {
            Ok(show) if !show.capabilities.is_empty() => ModelCaps {
                tools: show.capabilities.iter().any(|c| c == "tools"),
                thinking: show.capabilities.iter().any(|c| c == "thinking"),
            },
            _ => ModelCaps { tools: true, thinking: false },
        }
    }
}

/// Longueur du plus long suffixe de `buf` qui est un préfixe (partiel) de
/// `tag`. Sert à ne pas émettre un début de balise `<think>`/`</think>` coupé
/// entre deux chunks de streaming.
fn partial_tag_suffix(buf: &str, tag: &str) -> usize {
    let max = tag.len().min(buf.len());
    for len in (1..=max).rev() {
        let start = buf.len() - len;
        if buf.is_char_boundary(start) && tag.starts_with(&buf[start..]) {
            return len;
        }
    }
    0
}

/// Sépare un fragment de contenu streamé en segments de raisonnement
/// (`<think>...</think>`) et de réponse. Gère les balises coupées entre
/// chunks via le buffer `buf` et l'état `in_think`. Filet de sécurité pour les
/// modèles qui émettent le raisonnement inline dans `content` plutôt que dans
/// le champ `thinking` séparé.
fn split_think(fragment: &str, buf: &mut String, in_think: &mut bool) -> Vec<ChatEvent> {
    buf.push_str(fragment);
    let mut events = Vec::new();
    loop {
        if *in_think {
            if let Some(pos) = buf.find("</think>") {
                let thought: String = buf.drain(..pos).collect();
                buf.drain(.."</think>".len());
                if !thought.is_empty() {
                    events.push(ChatEvent::Thinking(thought));
                }
                *in_think = false;
            } else {
                let keep = partial_tag_suffix(buf, "</think>");
                let emit_len = buf.len() - keep;
                if emit_len > 0 {
                    let thought: String = buf.drain(..emit_len).collect();
                    events.push(ChatEvent::Thinking(thought));
                }
                break;
            }
        } else if let Some(pos) = buf.find("<think>") {
            let text: String = buf.drain(..pos).collect();
            buf.drain(.."<think>".len());
            if !text.is_empty() {
                events.push(ChatEvent::Token(text));
            }
            *in_think = true;
        } else {
            let keep = partial_tag_suffix(buf, "<think>");
            let emit_len = buf.len() - keep;
            if emit_len > 0 {
                let text: String = buf.drain(..emit_len).collect();
                events.push(ChatEvent::Token(text));
            }
            break;
        }
    }
    events
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

        // Détecte les capacités du modèle pour n'envoyer `tools`/`think` que
        // s'ils sont supportés (évite les HTTP 400 "does not support tools").
        let caps = self.model_capabilities(&req.model).await;

        let tools = if caps.tools {
            req.tools
                .iter()
                .map(|t| OllamaTool {
                    kind: "function".into(),
                    function: OllamaToolDef {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    },
                })
                .collect::<Vec<_>>()
        } else {
            if !req.tools.is_empty() {
                tracing::info!(
                    "Modèle {} ne supporte pas les tools — envoi sans outils",
                    req.model
                );
            }
            Vec::new()
        };

        let messages: Vec<OllamaMessage> = req
            .messages
            .iter()
            .map(|m| OllamaMessage {
                role: m.role.as_str().into(),
                content: m.content.clone(),
                images: Vec::new(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            })
            .collect();

        let body = OllamaChatRequest {
            model: req.model.clone(),
            messages,
            stream: true,
            think: if caps.thinking { Some(true) } else { None },
            options: OllamaOptions {
                temperature: req.temperature,
                num_predict: req.max_tokens,
            },
            tools,
        };

        let client = self.client.clone();

        let s = async_stream::stream! {
            // Première tentative.
            let mut body = body;
            let mut response = match client.post(&url).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    yield ChatEvent::Error(format!(
                        "Ollama injoignable sur {url} — vérifiez qu'Ollama tourne (ollama serve). Détail: {e}"
                    ));
                    yield ChatEvent::Done(Usage::default());
                    return;
                }
            };

            // Filet de sécurité : si la détection de capacités a échoué et que
            // le modèle refuse les tools, on retente une fois sans outils.
            if response.status() == reqwest::StatusCode::BAD_REQUEST && !body.tools.is_empty() {
                let text = response.text().await.unwrap_or_default();
                if text.contains("does not support tools") {
                    tracing::warn!("Modèle {} ne supporte pas les tools, nouvel essai sans", body.model);
                    body.tools.clear();
                    response = match client.post(&url).json(&body).send().await {
                        Ok(r) => r,
                        Err(e) => {
                            yield ChatEvent::Error(format!("Ollama injoignable: {e}"));
                            yield ChatEvent::Done(Usage::default());
                            return;
                        }
                    };
                } else {
                    yield ChatEvent::Error(format!("Ollama a répondu 400: {text}"));
                    yield ChatEvent::Done(Usage::default());
                    return;
                }
            }

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
            // État du parseur de balises <think> inline (fallback).
            let mut think_buf = String::new();
            let mut in_think = false;

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
                            let Ok(parsed) = serde_json::from_str::<OllamaChatChunk>(line_str) else {
                                tracing::debug!("chunk Ollama non parsé: {}", line_str);
                                continue
                            };
                            if let Some(msg) = parsed.message {
                                // Champ `thinking` séparé (modèles thinking).
                                if !msg.thinking.is_empty() {
                                    yield ChatEvent::Thinking(msg.thinking);
                                }
                                // Contenu : on sépare un éventuel <think> inline.
                                if !msg.content.is_empty() {
                                    for ev in split_think(&msg.content, &mut think_buf, &mut in_think) {
                                        yield ev;
                                    }
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
                                // Flush du reliquat éventuel du buffer <think>.
                                if !think_buf.is_empty() {
                                    if in_think {
                                        yield ChatEvent::Thinking(std::mem::take(&mut think_buf));
                                    } else {
                                        yield ChatEvent::Token(std::mem::take(&mut think_buf));
                                    }
                                }
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
            if !think_buf.is_empty() {
                yield ChatEvent::Token(std::mem::take(&mut think_buf));
            }
            yield ChatEvent::Done(usage);
        };

        s.boxed()
    }
}
