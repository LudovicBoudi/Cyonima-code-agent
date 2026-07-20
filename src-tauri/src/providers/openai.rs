//! Provider OpenAI — API distante `chat/completions` streaming SSE.
//!
//! Endpoint : `https://api.openai.com/v1/chat/completions` (override via
//! `OPENAI_BASE_URL`-like plus tard). Auth : `Authorization: Bearer <key>`.
//! Format tool-call : `tools` + `tool_calls` (function calling natif).
//!
//! Compatible GPT-4o, GPT-4.1, o1, o3, etc. — n'importe quel modèle qui
//! accepte le chat/completions OpenAI standard.

use std::time::Duration;

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::{Capabilities, ChatEvent, ChatRequest, Provider, Usage};

const DEFAULT_ENDPOINT: &str = "https://api.openai.com/v1";

pub struct OpenAiProvider {
    endpoint: String,
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(endpoint: Option<String>, api_key: String) -> Self {
        let endpoint = endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .expect("échec construction client HTTP OpenAI");
        Self { endpoint, api_key, client }
    }
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "max_tokens")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tools: Vec<OpenAiTool>,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    kind: String,
    function: OpenAiToolDef,
}

#[derive(Debug, Serialize)]
struct OpenAiToolDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: OpenAiDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiDeltaToolCall>,
}

#[derive(Debug, Deserialize)]
struct OpenAiDeltaToolCall {
    #[serde(default)]
    index: Option<u32>,
    #[serde(default)]
    function: OpenAiDeltaToolFn,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiDeltaToolFn {
    #[serde(default)]
    name: Option<String>,
    /// OpenAI renvoie les arguments par morceaux à streamer ; on les concatène.
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn id(&self) -> &str { "openai" }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_tools: true,
            supports_vision: true,
            context_window: 128_000,
        }
    }

    async fn stream(&self, req: ChatRequest) -> BoxStream<'static, ChatEvent> {
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));
        let body = OpenAiChatRequest {
            model: req.model.clone(),
            messages: req
                .messages
                .iter()
                .map(|m| OpenAiMessage { role: m.role.as_str().into(), content: m.content.clone() })
                .collect(),
            stream: true,
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            tools: req
                .tools
                .iter()
                .map(|t| OpenAiTool {
                    kind: "function".into(),
                    function: OpenAiToolDef {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    },
                })
                .collect(),
        };

        let client = self.client.clone();
        let auth = format!("Bearer {}", self.api_key);

        let s = async_stream::stream! {
            let response = match client
                .post(&url)
                .header("Authorization", &auth)
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    yield ChatEvent::Error(format!(
                        "OpenAI injoignable sur {url} — {e}. Vérifiez votre clé API et votre connexion réseau."
                    ));
                    yield ChatEvent::Done(Usage::default());
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                yield ChatEvent::Error(format!("OpenAI a répondu {status}: {text}"));
                yield ChatEvent::Done(Usage::default());
                return;
            }

            // SSE : lignes `data: {json}` ; terminaison `data: [DONE]`.
            let mut bytes_stream = response.bytes_stream();
            let mut buffer: Vec<u8> = Vec::new();
            // Accumulateur d'arguments par index tool_call (OpenAI stream par fragments).
            let mut tool_args: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
            let mut tool_names: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
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
                            if line == "data: [DONE]" {
                                yield ChatEvent::Done(usage);
                                return;
                            }
                            let Some(json) = line.strip_prefix("data: ") else { continue };
                            let Ok(parsed) = serde_json::from_str::<OpenAiStreamChunk>(json) else { continue };
                            if let Some(u) = parsed.usage {
                                usage.tokens_in = u.prompt_tokens;
                                usage.tokens_out = u.completion_tokens;
                            }
                            for choice in parsed.choices {
                                if let Some(c) = choice.delta.content {
                                    if !c.is_empty() {
                                        yield ChatEvent::Token(c);
                                    }
                                }
                                for tc in choice.delta.tool_calls {
                                    let i = tc.index.unwrap_or(0);
                                    if let Some(name) = tc.function.name {
                                        if !name.is_empty() {
                                            tool_names.insert(i, name);
                                        }
                                    }
                                    if let Some(args) = tc.function.arguments {
                                        if !args.is_empty() {
                                            let acc = tool_args.entry(i).or_default();
                                            acc.push_str(&args);
                                        }
                                    }
                                }
                                // OpenAI ne fournit pas d'événement explicite « tool_call done »,
                                // on prend comme règle empirique que `finish_reason == "tool_calls"`
                                // remplit l'accumulateur → on yield chaque tool_call.
                                if choice.finish_reason.as_deref() == Some("tool_calls") {
                                    for i in 0..16u32 {
                                    let Some(name) = tool_names.get(&i) else { break };
                                    let args_str = tool_args.remove(&i).unwrap_or_default();
                                    let parsed = serde_json::from_str(&args_str).unwrap_or(serde_json::Value::Null);
                                    yield ChatEvent::ToolCall(super::ToolCall {
                                        id: format!("call_{i}_{}", uuid::Uuid::new_v4()),
                                        tool: name.clone(),
                                        arguments: parsed,
                                    });
                                    tool_names.remove(&i);
                                }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield ChatEvent::Error(format!("flux OpenAI interrompu: {e}"));
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