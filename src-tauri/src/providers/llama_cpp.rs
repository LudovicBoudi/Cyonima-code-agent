//! Provider `llama_cpp` — inférence locale built-in via candle (GGUF CPU).
//!
//! **Statut J1** : stub conforme au trait `Provider`. Le câblage réel des
//! bindings candle (`candle-core` + `candle-transformers::models::quantized_*`)
//! arrive en **J1.5** : on pourra alors tester contre un vrai GGUF sans dépendre
//! d'Ollama. Tant que le câblage n'est pas en place, ce provider émet un
//! `ChatEvent::Error` clair invitant à utiliser le provider `ollama` ou à
//! attendre J1.5.
//!
//! Raison du report : sans GGUF téléchargeable côté app (le downloader du J4
//! n'est pas encore livré) ni la possibilité de boucler un test end-to-end en
//! local ce jalon, livrer candle "au doigt mouillé" violerait la règle
//! d'AGENTS.md ("run lint/typecheck and fix what you modify"). On implémente
//! donc le squelette, et on branche la vraie inférence dès qu'on pourra la
//! valider contre un modèle réel.

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;

use super::{Capabilities, ChatEvent, ChatRequest, Provider};

pub struct LlamaCppProvider {
    /// Chemin du GGUF. Renseigné par le session manager quand on saura where.
    _model_path: Option<String>,
}

impl Default for LlamaCppProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LlamaCppProvider {
    pub fn new() -> Self {
        Self { _model_path: None }
    }

    pub fn with_model_path(path: impl Into<String>) -> Self {
        Self {
            _model_path: Some(path.into()),
        }
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
            context_window: 4096,
        }
    }

    async fn stream(&self, _req: ChatRequest) -> BoxStream<'static, ChatEvent> {
        let msg = String::from(
            "Inférence locale built-in (candle) non câblée en J1 — \
             utilisez le provider ollama pour tester dès maintenant. \
             Le backend local 100% offline arrive en J1.5.",
        );
        futures::stream::once(async move { ChatEvent::Error(msg) })
            .chain(futures::stream::once(async {
                ChatEvent::Done(super::Usage::default())
            }))
            .boxed()
    }
}
