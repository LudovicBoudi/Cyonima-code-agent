//! Provider OpenAI-compatible — endpoint custom (LM Studio, vLLM, entreprise).
//!
//! Réutilise la même logique SSE que [`super::openai::OpenAiProvider`] mais
//! avec un endpoint **obligatoirement** fourni par l'utilisateur (pas de
//! défaut `https://api.openai.com`). La clé API est optionnelle (LM Studio
//! en local n'en demande pas).
//!
//! Compatible : LM Studio (http://localhost:1234/v1), vLLM, TGI, OpenRouter,
//! Azure OpenAI (avec endpoint custom), Mistral La Plateforme, etc.

use std::time::Duration;

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::{Capabilities, ChatEvent, ChatRequest, Provider, Usage};

pub struct OpenAiCompatProvider {
    endpoint: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl OpenAiCompatProvider {
    pub fn new(endpoint: String, api_key: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .expect("échec construction client HTTP OpenAI-compat");
        Self { endpoint, api_key, client }
    }
}

// On réutilise exactement le format OpenAI (LM Studio, vLLM… embarquent tous
// le même format SSE /chat/completions). On duplique la struct plutôt que de
// `pub(crate)`-iser celles de super::openai pour rester lâche: certains serveurs
// (vLLM ancien) n'implémentent pas `usage` ou `tool_calls` — on les mark `default`.

#[derive(Debug, Serialize)]
struct CompatChatRequest {
    model: String,
    messages: Vec<CompatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "max_tokens")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tools: Vec<CompatTool>,
}

#[derive(Debug, Serialize)]
struct CompatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct CompatTool {
    #[serde(rename = "type")]
    kind: String,
    function: CompatToolDef,
}

#[derive(Debug, Serialize)]
struct CompatToolDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct CompatStreamChunk {
    choices: Vec<CompatStreamChoice>,
    #[serde(default)]
    usage: Option<CompatUsage>,
}

#[derive(Debug, Deserialize)]
struct CompatStreamChoice {
    #[serde(default)]
    delta: CompatDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CompatDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<CompatDeltaToolCall>,
}

#[derive(Debug, Deserialize)]
struct CompatDeltaToolCall {
    #[serde(default)]
    index: Option<u32>,
    #[serde(default)]
    function: CompatDeltaToolFn,
}

#[derive(Debug, Default, Deserialize)]
struct CompatDeltaToolFn {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[async_trait]
impl Provider for OpenAiCompatProvider {
    fn id(&self) -> &str { "openai_compat" }

    fn capabilities(&self) -> Capabilities {
        // LM Studio / vLLM : tool calling dépend du modèle servé. On déclare
        // `supports_tools = true` par défaut car le format est accepté, mais
        // l'utilisateur est maître du modèle qui tourne derrière.
        Capabilities {
            supports_tools: true,
            supports_vision: false, // dépend : conservateur
            context_window: 8_192,
        }
    }

    async fn stream(&self, req: ChatRequest) -> BoxStream<'static, ChatEvent> {
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));
        let body = CompatChatRequest {
            model: req.model.clone(),
            messages: req
                .messages
                .iter()
                .map(|m| CompatMessage { role: m.role.as_str().into(), content: m.content.clone() })
                .collect(),
            stream: true,
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            tools: req
                .tools
                .iter()
                .map(|t| CompatTool {
                    kind: "function".into(),
                    function: CompatToolDef {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    },
                })
                .collect(),
        };

        let client = self.client.clone();
        let auth = self.api_key.clone();

        let s = async_stream::stream! {
            let mut req_builder = client.post(&url);
            if let Some(key) = &auth {
                if !key.is_empty() {
                    req_builder = req_builder.header("Authorization", format!("Bearer {key}"));
                }
            }
            let response = match req_builder.json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    yield ChatEvent::Error(format!(
                        "endoint OpenAI-compat injoignable sur {url} — {e}. Vérifiez l'URL et que le serveur tourne."
                    ));
                    yield ChatEvent::Done(Usage::default());
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                yield ChatEvent::Error(format!("endpoint a répondu {status}: {text}"));
                yield ChatEvent::Done(Usage::default());
                return;
            }

            let mut bytes_stream = response.bytes_stream();
            let mut buffer: Vec<u8> = Vec::new();
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
                            let Ok(parsed) = serde_json::from_str::<CompatStreamChunk>(json) else { continue };
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
                        yield ChatEvent::Error(format!("flux interrompu: {e}"));
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