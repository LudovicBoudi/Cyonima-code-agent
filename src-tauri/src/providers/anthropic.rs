//! Provider Anthropic — API distante `/v1/messages` streaming SSE.
//!
//! Endpoint : `https://api.anthropic.com/v1/messages`. Auth :
//! `x-api-key: <key>` + `anthropic-version: 2023-06-01`. Format tool-call
//! natif via `tools` + `tool_use` / `tool_result` blocks.
//!
//! Compatible Claude 3.5 Sonnet, Claude 3.7 Sonnet, Claude 4 Opus, etc.

use std::time::Duration;

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::{Capabilities, ChatEvent, ChatRequest, Provider, Usage};

const DEFAULT_ENDPOINT: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    endpoint: String,
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(endpoint: Option<String>, api_key: String) -> Self {
        let endpoint = endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .expect("échec construction client HTTP Anthropic");
        Self { endpoint, api_key, client }
    }
}

#[derive(Debug, Serialize)]
struct AnthropicMessagesRequest {
    model: String,
    /// Anthropic exige un `system` top-level (pas en messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "max_tokens")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tools: Vec<AnthropicTool>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

// --- SSE event parsing ----

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicEvent {
    #[serde(rename = "message_start")]
    MessageStart,
    #[serde(rename = "content_block_start")]
    ContentBlockStart { index: u32, content_block: AnthropicContentBlock },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: u32, delta: AnthropicDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },
    #[serde(rename = "message_delta")]
    MessageDelta { usage: Option<AnthropicUsage> },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: AnthropicErrorBody },
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<serde_json::Value>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorBody {
    message: String,
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn id(&self) -> &str { "anthropic" }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_tools: true,
            supports_vision: true,
            context_window: 200_000,
        }
    }

    async fn stream(&self, req: ChatRequest) -> BoxStream<'static, ChatEvent> {
        let url = format!("{}/messages", self.endpoint.trim_end_matches('/'));

        // Anthropic sépare les messages système du reste. On extrait.
        let mut system_prompt: Option<String> = None;
        let mut filtered_messages: Vec<super::ChatMessage> = Vec::new();
        for m in &req.messages {
            if m.role == super::Role::System {
                system_prompt = Some(m.content.clone());
            } else {
                filtered_messages.push(m.clone());
            }
        }

        let body = AnthropicMessagesRequest {
            model: req.model.clone(),
            system: system_prompt,
            messages: filtered_messages
                .iter()
                .map(|m| AnthropicMessage {
                    role: m.role.as_str().into(),
                    content: serde_json::Value::String(m.content.clone()),
                })
                .collect(),
            stream: true,
            temperature: req.temperature,
            max_tokens: req.max_tokens.or(Some(4096)),
            tools: req
                .tools
                .iter()
                .map(|t| AnthropicTool {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: t.parameters.clone(),
                })
                .collect(),
        };

        let client = self.client.clone();
        let api_key = self.api_key.clone();

        let s = async_stream::stream! {
            let response = match client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    yield ChatEvent::Error(format!(
                        "Anthropic injoignable sur {url} — {e}. Vérifiez votre clé API et votre connexion réseau."
                    ));
                    yield ChatEvent::Done(Usage::default());
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                yield ChatEvent::Error(format!("Anthropic a répondu {status}: {text}"));
                yield ChatEvent::Done(Usage::default());
                return;
            }

            // SSE : lignes `event: <name>` puis `data: {json}`.
            let mut bytes_stream = response.bytes_stream();
            let mut buffer: Vec<u8> = Vec::new();
            // Accumulateur d'arguments du content_block tool_use en cours.
            let mut tool_args: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
            let mut tool_names: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
            let mut tool_ids: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
            let mut usage = Usage::default();

            while let Some(chunk_res) = bytes_stream.next().await {
                match chunk_res {
                    Ok(bytes) => {
                        buffer.extend_from_slice(&bytes);
                        loop {
                            let Some(nl) = buffer.iter().position(|b| *b == b'\n') else { break };
                            let line: Vec<u8> = buffer.drain(..=nl).collect();
                            let line = std::str::from_utf8(&line).unwrap_or("").trim().to_string();
                            if line.is_empty() { continue; }
                            let Some(json) = line.strip_prefix("data: ") else { continue };
                            let Ok(parsed) = serde_json::from_str::<AnthropicEvent>(json) else { continue };
                            match parsed {
                                AnthropicEvent::ContentBlockStart { index, content_block } => {
                                    if content_block.kind.as_deref() == Some("tool_use") {
                                        if let Some(n) = content_block.name {
                                            tool_names.insert(index, n);
                                        }
                                        if let Some(id) = content_block.id {
                                            tool_ids.insert(index, id);
                                        }
                                        tool_args.entry(index).or_default();
                                    }
                                }
                                AnthropicEvent::ContentBlockDelta { index, delta } => match delta {
                                    AnthropicDelta::TextDelta { text } => {
                                        if !text.is_empty() {
                                            yield ChatEvent::Token(text);
                                        }
                                    }
                                    AnthropicDelta::InputJsonDelta { partial_json } => {
                                        if let Some(acc) = tool_args.get_mut(&index) {
                                            acc.push_str(&partial_json);
                                        }
                                    }
                                },
                                AnthropicEvent::ContentBlockStop { index } => {
                                    if let Some(name) = tool_names.remove(&index) {
                                        let args_str = tool_args.remove(&index).unwrap_or_default();
                                        let _ = tool_ids.remove(&index);
                                        let parsed = serde_json::from_str(&args_str).unwrap_or(serde_json::Value::Null);
                                        yield ChatEvent::ToolCall(super::ToolCall {
                                            id: format!("call_{index}_{}", uuid::Uuid::new_v4()),
                                            tool: name,
                                            arguments: parsed,
                                        });
                                    }
                                }
                                AnthropicEvent::MessageDelta { usage: Some(u) } => {
                                    usage.tokens_in = u.input_tokens;
                                    usage.tokens_out = u.output_tokens;
                                }
                                AnthropicEvent::MessageStop => {
                                    yield ChatEvent::Done(usage);
                                    return;
                                }
                                AnthropicEvent::Error { error } => {
                                    yield ChatEvent::Error(error.message);
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        yield ChatEvent::Error(format!("flux Anthropic interrompu: {e}"));
                        yield ChatEvent::Done(usage);
                        return;
                    }
                }
            }
            yield ChatEvent::Done(usage);
        };

        s.boxed()
    }
}