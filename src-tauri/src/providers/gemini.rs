//! Provider Google Gemini — API distante `/v1beta/models/{model}:streamGenerateContent`.
//!
//! Endpoint : `https://generativelanguage.googleapis.com/v1beta`. Auth :
//! `x-goog-api-key: <key>` ou query param `?key=<key>`. Format streaming
//! via `alt=sse` (Server-Sent Events).
//!
//! Compatible Gemini 1.5 Flash/Pro, Gemini 2.0 Flash, Gemini 2.5 Pro.
//!
//! remarque : Gemini n'offre pas le tool-calling en streaming SSE natif dans
//! tous les modèles. On expose donc tools mais le parsing des
//! `functionCall` reste basique.

use std::time::Duration;

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::{Capabilities, ChatEvent, ChatRequest, Provider, Usage};

const DEFAULT_ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct GeminiProvider {
    endpoint: String,
    api_key: String,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(endpoint: Option<String>, api_key: String) -> Self {
        let endpoint = endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .expect("échec construction client HTTP Gemini");
        Self {
            endpoint,
            api_key,
            client,
        }
    }
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiReqContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiReqContent>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "generationConfig")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tools: Vec<GeminiTool>,
}

#[derive(Debug, Serialize)]
struct GeminiReqContent {
    role: String,
    parts: Vec<GeminiReqPart>,
}

#[derive(Debug, Serialize)]
struct GeminiReqPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiResContent {
    #[allow(dead_code)]
    role: String,
    parts: Vec<GeminiResPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiResPart {
    #[serde(default)]
    text: String,
    #[serde(default, rename = "functionCall")]
    function_call: Option<GeminiFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxOutputTokens")]
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFnDecl>,
}

#[derive(Debug, Serialize)]
struct GeminiFnDecl {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct GeminiStreamChunk {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    #[serde(default)]
    content: Option<GeminiResContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiUsage {
    #[serde(default, rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(default, rename = "candidatesTokenCount")]
    candidates_token_count: u32,
}

#[async_trait]
impl Provider for GeminiProvider {
    fn id(&self) -> &str {
        "gemini"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_tools: true,
            supports_vision: true,
            context_window: 1_000_000, // Gemini 1.5+ supporte 1M
        }
    }

    async fn stream(&self, req: ChatRequest) -> BoxStream<'static, ChatEvent> {
        let model = req.model.clone();
        // Gemini veut le nom du modèle dans l'URL : `models/gemini-1.5-flash`.
        // Si l'utilisateur passe "gemini-1.5-flash" sans prefix, on l'ajoute.
        let model_path = if model.starts_with("models/") {
            model.clone()
        } else {
            format!("models/{model}")
        };
        let url = format!(
            "{}/{model_path}:streamGenerateContent?alt=sse",
            self.endpoint.trim_end_matches('/')
        );

        // Séparation system vs conversation (Gemini utilise system_instruction).
        let mut system_parts: Vec<GeminiReqPart> = Vec::new();
        let mut contents: Vec<GeminiReqContent> = Vec::new();
        for m in &req.messages {
            match m.role {
                super::Role::System => {
                    system_parts.push(GeminiReqPart {
                        text: Some(m.content.clone()),
                    });
                }
                other => {
                    // Gemini utilise "user" et "model" (pas "assistant").
                    let role = match other {
                        super::Role::Assistant => "model".into(),
                        super::Role::Tool => "user".into(), // pas de role tool natif, on inline
                        _ => other.as_str().into(),
                    };
                    contents.push(GeminiReqContent {
                        role,
                        parts: vec![GeminiReqPart {
                            text: Some(m.content.clone()),
                        }],
                    });
                }
            }
        }

        let body = GeminiRequest {
            contents,
            system_instruction: if system_parts.is_empty() {
                None
            } else {
                Some(GeminiReqContent {
                    role: "user".into(),
                    parts: system_parts,
                })
            },
            generation_config: Some(GeminiGenerationConfig {
                temperature: req.temperature,
                max_output_tokens: req.max_tokens,
            }),
            tools: req
                .tools
                .iter()
                .map(|t| GeminiTool {
                    function_declarations: vec![GeminiFnDecl {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    }],
                })
                .collect(),
        };

        let client = self.client.clone();
        let api_key = self.api_key.clone();

        let s = async_stream::stream! {
            let response = match client
                .post(&url)
                .header("x-goog-api-key", &api_key)
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    yield ChatEvent::Error(format!(
                        "Gemini injoignable sur {url} — {e}. Vérifiez votre clé API et votre connexion réseau."
                    ));
                    yield ChatEvent::Done(Usage::default());
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                yield ChatEvent::Error(format!("Gemini a répondu {status}: {text}"));
                yield ChatEvent::Done(Usage::default());
                return;
            }

            // SSE Gemini : `data: {json}`.
            let mut bytes_stream = response.bytes_stream();
            let mut buffer: Vec<u8> = Vec::new();
            let mut usage = Usage::default();

            while let Some(chunk_res) = bytes_stream.next().await {
                match chunk_res {
                    Ok(bytes) => {
                        buffer.extend_from_slice(&bytes);
                        while let Some(nl) = buffer.iter().position(|b| *b == b'\n') {
                            let line: Vec<u8> = buffer.drain(..=nl).collect();
                            let line = std::str::from_utf8(&line).unwrap_or("").trim().to_string();
                            if line.is_empty() { continue; }
                            let Some(json) = line.strip_prefix("data: ") else { continue };
                            let Ok(parsed) = serde_json::from_str::<GeminiStreamChunk>(json) else { continue };
                            if let Some(u) = parsed.usage_metadata {
                                usage.tokens_in = u.prompt_token_count;
                                usage.tokens_out = u.candidates_token_count;
                            }
                            for c in parsed.candidates {
                                if let Some(content) = c.content {
                                    for part in content.parts {
                                        if !part.text.is_empty() {
                                            yield ChatEvent::Token(part.text);
                                        }
                                        if let Some(fc) = part.function_call {
                                            yield ChatEvent::ToolCall(super::ToolCall {
                                                id: format!("gemini_call_{}", uuid::Uuid::new_v4()),
                                                tool: fc.name,
                                                arguments: fc.args,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield ChatEvent::Error(format!("flux Gemini interrompu: {e}"));
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
