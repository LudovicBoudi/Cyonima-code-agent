//! Provider `llama_cpp` — inférence locale built-in via candle (GGUF CPU).
//!
//! Charge un fichier GGUF, détecte l'architecture (llama, gemma3, qwen2, phi3),
//! instancie le modèle quantizé correspondant via `candle-transformers`, puis
//! tourne la boucle de génération token-par-token en streaming.
//!
//! Le tokenizer BPE est reconstruit à partir des métadonnées GGUF embarquées
//! (via `TokenizerFromGguf` de `candle-core`), sans fichier externe.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use candle_core::quantized::gguf_file;
use candle_core::quantized::tokenizer::TokenizerFromGguf;
use candle_core::Device;
use futures::stream::BoxStream;
use futures::StreamExt;
use tokenizers::Tokenizer;

use super::{Capabilities, ChatEvent, ChatRequest, Provider, Usage};

// ---------------------------------------------------------------------------
// GgufModel — trait d'abstraction pour les modèles quantizés candle
// ---------------------------------------------------------------------------

trait GgufModel: Send {
    fn forward(&mut self, tokens: &[u32], index_pos: usize) -> Result<Vec<f32>, String>;
    fn clear_kv_cache(&mut self);
}

// --- LLaMA ---------------------------------------------------------------

struct LlamaModel(candle_transformers::models::quantized_llama::ModelWeights);

impl GgufModel for LlamaModel {
    fn forward(&mut self, tokens: &[u32], index_pos: usize) -> Result<Vec<f32>, String> {
        let input = candle_core::Tensor::new(tokens, &Device::Cpu)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| e.to_string())?;
        self.0
            .forward(&input, index_pos)
            .and_then(|logits| logits.squeeze(0))
            .and_then(|l| l.to_vec1::<f32>())
            .map_err(|e| e.to_string())
    }

    fn clear_kv_cache(&mut self) {
        self.0.clear_kv_cache();
    }
}

// --- Gemma 3 -------------------------------------------------------------

struct Gemma3Model(candle_transformers::models::quantized_gemma3::ModelWeights);

impl GgufModel for Gemma3Model {
    fn forward(&mut self, tokens: &[u32], index_pos: usize) -> Result<Vec<f32>, String> {
        let input = candle_core::Tensor::new(tokens, &Device::Cpu)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| e.to_string())?;
        self.0
            .forward(&input, index_pos)
            .and_then(|logits| logits.squeeze(0))
            .and_then(|l| l.to_vec1::<f32>())
            .map_err(|e| e.to_string())
    }

    fn clear_kv_cache(&mut self) {}
}

// --- Qwen 2 -------------------------------------------------------------

struct Qwen2Model(candle_transformers::models::quantized_qwen2::ModelWeights);

impl GgufModel for Qwen2Model {
    fn forward(&mut self, tokens: &[u32], index_pos: usize) -> Result<Vec<f32>, String> {
        let input = candle_core::Tensor::new(tokens, &Device::Cpu)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| e.to_string())?;
        self.0
            .forward(&input, index_pos)
            .and_then(|logits| logits.squeeze(0))
            .and_then(|l| l.to_vec1::<f32>())
            .map_err(|e| e.to_string())
    }

    fn clear_kv_cache(&mut self) {
        self.0.clear_kv_cache();
    }
}

// --- Phi 3 ---------------------------------------------------------------

struct Phi3Model(candle_transformers::models::quantized_phi3::ModelWeights);

impl GgufModel for Phi3Model {
    fn forward(&mut self, tokens: &[u32], index_pos: usize) -> Result<Vec<f32>, String> {
        let input = candle_core::Tensor::new(tokens, &Device::Cpu)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| e.to_string())?;
        self.0
            .forward(&input, index_pos)
            .and_then(|logits| logits.squeeze(0))
            .and_then(|l| l.to_vec1::<f32>())
            .map_err(|e| e.to_string())
    }

    fn clear_kv_cache(&mut self) {}
}

// ---------------------------------------------------------------------------
// État du modèle chargé
// ---------------------------------------------------------------------------

struct GgufModelState {
    model: Box<dyn GgufModel>,
    tokenizer: Tokenizer,
    eos_token_id: u32,
    context_window: usize,
}

// ---------------------------------------------------------------------------
// Chargement GGUF
// ---------------------------------------------------------------------------

fn load_gguf<P: AsRef<Path>>(path: P) -> Result<GgufModelState, String> {
    let path = path.as_ref();
    let mut file = std::fs::File::open(path).map_err(|e| format!("ouverture GGUF: {e}"))?;
    let content =
        gguf_file::Content::read(&mut file).map_err(|e| format!("lecture GGUF: {e}"))?;

    // Extraire les métadonnées avant de consommer `content`.
    let arch = content
        .metadata
        .get("general.architecture")
        .and_then(|v| v.to_string().ok())
        .cloned()
        .unwrap_or_default();

    let eos_token_id = content
        .metadata
        .get("tokenizer.ggml.eos_token_id")
        .and_then(|v| v.to_u32().ok())
        .unwrap_or(2);

    let context_window = content
        .metadata
        .get("llama.context_length")
        .and_then(|v| v.to_u32().ok())
        .unwrap_or(4096) as usize;

    let tokenizer =
        Tokenizer::from_gguf(&content).map_err(|e| format!("tokenizer GGUF: {e}"))?;

    let device = Device::Cpu;

    // Charger le modèle (consomme `content` — un seul arch possible).
    let mut model: Box<dyn GgufModel> = match arch.as_str() {
        "gemma3" => {
            let inner =
                candle_transformers::models::quantized_gemma3::ModelWeights::from_gguf(
                    content,
                    &mut file,
                    &device,
                )
                .map_err(|e| format!("chargement gemma3: {e}"))?;
            Box::new(Gemma3Model(inner))
        }
        "qwen2" => {
            let inner =
                candle_transformers::models::quantized_qwen2::ModelWeights::from_gguf(
                    content,
                    &mut file,
                    &device,
                )
                .map_err(|e| format!("chargement qwen2: {e}"))?;
            Box::new(Qwen2Model(inner))
        }
        "phi3" => {
            let inner =
                candle_transformers::models::quantized_phi3::ModelWeights::from_gguf(
                    false, // use_flash_attn
                    content,
                    &mut file,
                    &device,
                )
                .map_err(|e| format!("chargement phi3: {e}"))?;
            Box::new(Phi3Model(inner))
        }
        _ => {
            let inner =
                candle_transformers::models::quantized_llama::ModelWeights::from_gguf(
                    content,
                    &mut file,
                    &device,
                )
                .map_err(|e| format!("chargement llama: {e}"))?;
            Box::new(LlamaModel(inner))
        }
    };

    model.clear_kv_cache();

    Ok(GgufModelState {
        model,
        tokenizer,
        eos_token_id,
        context_window,
    })
}

// ---------------------------------------------------------------------------
// Génération token-par-token (appelé dans un blocking task)
// ---------------------------------------------------------------------------

fn generate_tokens(
    state: &mut GgufModelState,
    prompt_tokens: &[u32],
    max_tokens: usize,
    temperature: f32,
) -> Result<(Vec<String>, usize), String> {
    use candle_transformers::generation::LogitsProcessor;

    let prompt_len = prompt_tokens.len();
    let temp = if temperature <= 0.0 {
        None
    } else {
        Some(temperature as f64)
    };
    let mut logits_processor = LogitsProcessor::new(42, temp, None);

    let mut index_pos = 0;
    let mut all_texts: Vec<String> = Vec::new();

    // Forward pass sur le prompt complet.
    let logits = state.model.forward(prompt_tokens, index_pos)?;
    index_pos += prompt_len;

    // Convertir Vec<f32> en Tensor pour le sampling.
    let logits_tensor = candle_core::Tensor::new(logits.as_slice(), &Device::Cpu)
        .map_err(|e| e.to_string())?;
    let mut next_token = logits_processor
        .sample(&logits_tensor)
        .map_err(|e| format!("sampling: {e}"))?;

    let first_text = state
        .tokenizer
        .decode(&[next_token], true)
        .unwrap_or_default();
    all_texts.push(first_text);

    // Boucle de génération.
    for _ in 0..max_tokens.saturating_sub(1) {
        if next_token == state.eos_token_id {
            break;
        }

        let logits = state.model.forward(&[next_token], index_pos)?;
        index_pos += 1;

        let logits_tensor = candle_core::Tensor::new(logits.as_slice(), &Device::Cpu)
            .map_err(|e| e.to_string())?;
        next_token = logits_processor
            .sample(&logits_tensor)
            .map_err(|e| format!("sampling: {e}"))?;

        let text = state
            .tokenizer
            .decode(&[next_token], true)
            .unwrap_or_default();
        all_texts.push(text);
    }

    Ok((all_texts, prompt_len))
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub struct LlamaCppProvider {
    state: Option<Arc<tokio::sync::Mutex<GgufModelState>>>,
    context_window: usize,
}

impl Default for LlamaCppProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LlamaCppProvider {
    pub fn new() -> Self {
        Self {
            state: None,
            context_window: 4096,
        }
    }

    pub fn with_model_path(path: impl Into<String>) -> Self {
        let path = path.into();
        match load_gguf(&path) {
            Ok(state) => {
                let context_window = state.context_window;
                Self {
                    state: Some(Arc::new(tokio::sync::Mutex::new(state))),
                    context_window,
                }
            }
            Err(e) => {
                tracing::error!("échec chargement GGUF {path}: {e}");
                Self::new()
            }
        }
    }

    fn format_prompt(req: &ChatRequest) -> String {
        let mut prompt = String::new();
        for msg in &req.messages {
            match msg.role {
                super::Role::System => {
                    prompt.push_str(&format!("<|system|>\n{}\n", msg.content));
                }
                super::Role::User => {
                    prompt.push_str(&format!("<|user|>\n{}\n", msg.content));
                }
                super::Role::Assistant => {
                    prompt.push_str(&format!("<|assistant|>\n{}\n", msg.content));
                }
                super::Role::Tool => {
                    prompt.push_str(&format!("<|tool|>\n{}\n", msg.content));
                }
            }
        }
        prompt.push_str("<|assistant|>\n");
        prompt
    }
}

#[async_trait]
impl Provider for LlamaCppProvider {
    fn id(&self) -> &str {
        "llama_cpp"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_tools: false,
            supports_vision: false,
            context_window: self.context_window as u32,
        }
    }

    async fn stream(&self, req: ChatRequest) -> BoxStream<'static, ChatEvent> {
        let Some(state) = self.state.clone() else {
            let msg = String::from(
                "Aucun modèle GGUF chargé. Importez un fichier .gguf via \
                 le catalogue ou la page Import pour activer l'inférence locale.",
            );
            return futures::stream::once(async move { ChatEvent::Error(msg) })
                .chain(futures::stream::once(async {
                    ChatEvent::Done(Usage::default())
                }))
                .boxed();
        };

        let prompt = Self::format_prompt(&req);
        let max_tokens = req.max_tokens.unwrap_or(2048) as usize;
        let temperature = req.temperature.unwrap_or(0.7);

        let (tx, rx) = tokio::sync::mpsc::channel::<ChatEvent>(256);

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            let mut guard = rt.block_on(state.lock());

            let encoding = match guard.tokenizer.encode(prompt.as_str(), true) {
                Ok(e) => e,
                Err(e) => {
                    let _ = tx.blocking_send(ChatEvent::Error(format!("tokenisation: {e}")));
                    let _ = tx.blocking_send(ChatEvent::Done(Usage::default()));
                    return;
                }
            };
            let prompt_tokens = encoding.get_ids();

            if prompt_tokens.is_empty() {
                let _ = tx.blocking_send(ChatEvent::Error("prompt vide".into()));
                let _ = tx.blocking_send(ChatEvent::Done(Usage::default()));
                return;
            }

            match generate_tokens(&mut guard, prompt_tokens, max_tokens, temperature) {
                Ok((texts, prompt_len)) => {
                    let mut tokens_out = 0u32;
                    for text in &texts {
                        let _ = tx.blocking_send(ChatEvent::Token(text.clone()));
                        tokens_out += 1;
                    }
                    let _ = tx.blocking_send(ChatEvent::Done(Usage {
                        tokens_in: prompt_len as u32,
                        tokens_out,
                    }));
                }
                Err(e) => {
                    let _ = tx.blocking_send(ChatEvent::Error(e));
                    let _ = tx.blocking_send(ChatEvent::Done(Usage::default()));
                }
            }
        });

        tokio_stream::wrappers::ReceiverStream::new(rx).boxed()
    }
}
