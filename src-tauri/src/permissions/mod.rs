//! Gateway de permissions — point de passage unique pour tous les outils
//! modifiants (`write_file`, `edit_file`, `bash`). Pas de bypass.
//!
//! Flux :
//! 1. La session demande à exécuter un outil via [`Gateway::request`].
//! 2. Le gateway regarde la `Policy` applicable :
//!    - `Auto`  → `Decision::Allow` immédiat
//!    - `Deny`   → `Decision::Deny` immédiat
//!    - `Ask`    → émet l'event Tauri `permission:request` et bloque sur un
//!      `oneshot` jusqu'à l'appel IPC [`Gateway::respond`].
//! 3. La décision (Allow ou Deny) est renvoyée au session manager, qui ne
//!    exécute l'outil que si `Allow`.
//!
//! Defaults prudents : `read_file`/`glob`/`grep` = `Auto`, sinon `Ask`. Voir
//! [`default_policy`]. Override par config project/global viendra en J9.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Policy {
    Auto,
    #[default]
    Ask,
    Deny,
}

/// Defaults prudents : l'écriture et le shell demandent confirmation à
/// chaque fois ; la lecture / la recherche sont en auto-approve.
pub fn default_policy(tool: &str) -> Policy {
    match tool {
        "read_file" | "glob" | "grep" => Policy::Auto,
        "write_file" | "edit_file" | "bash" => Policy::Ask,
        _ => Policy::Ask,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionRequest {
    pub request_id: String,
    pub session_id: String,
    pub tool: String,
    /// Arguments JSON transmis par le LLM. Sont affichés tels quels côté UI.
    pub arguments: serde_json::Value,
    /// Prévisualisation humain-lisible (ex: `Commande: cargo build`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub preview: Option<String>,
}

struct Pending {
    tx: oneshot::Sender<Decision>,
    // Champ réservé pour futur catalogue "pending requests" exposé à l'UI
    // (annuler toutes les demandes en attente, etc.) — actuellement non
    // lu côté backend mais conservé pour ne pas casser la struct plus tard.
    #[allow(dead_code)]
    request: PermissionRequest,
}

/// Gateway partagée via l'`AppState`.
#[derive(Default)]
pub struct Gateway {
    pending: Arc<Mutex<HashMap<String, Pending>>>,
}

impl Gateway {
    pub fn new() -> Self {
        Self::default()
    }

    /// Renvoie la décision pour un tool-call. Bloque si la policy est `Ask`
    /// jusqu'à ce que l'utilisateur réponde via IPC [`respond`].
    pub async fn request(
        &self,
        app: AppHandle,
        session_id: String,
        tool: &str,
        args: serde_json::Value,
    ) -> Decision {
        self.request_with_policy(app, session_id, tool, default_policy(tool), args)
            .await
    }

    /// Variante qui accepte une policy explicite (utile pour les tests et
    /// les overrides de config à venir).
    pub async fn request_with_policy(
        &self,
        app: AppHandle,
        session_id: String,
        tool: &str,
        policy: Policy,
        args: serde_json::Value,
    ) -> Decision {
        match policy {
            Policy::Auto => Decision::Allow,
            Policy::Deny => Decision::Deny,
            Policy::Ask => {
                let (tx, rx) = oneshot::channel();
                let id = Uuid::new_v4().to_string();
                let preview = make_preview(tool, &args);
                let request = PermissionRequest {
                    request_id: id.clone(),
                    session_id,
                    tool: tool.into(),
                    arguments: args,
                    preview,
                };
                {
                    let mut guard = self.pending.lock().await;
                    guard.insert(
                        id.clone(),
                        Pending {
                            tx,
                            request: request.clone(),
                        },
                    );
                }
                let _ = app.emit("permission:request", &request);
                let decision = rx.await.unwrap_or(Decision::Deny);
                // Au cas où le frontend répondrait deux fois, on nettoie.
                self.pending.lock().await.remove(&id);
                decision
            }
        }
    }

    /// Appelé par la commande IPC `permission_respond`.
    pub async fn respond(&self, request_id: &str, decision: Decision) {
        let removed = self.pending.lock().await.remove(request_id);
        if let Some(p) = removed {
            let _ = p.tx.send(decision);
        }
    }
}

/// Construit une prévisualisation humaine-lisible des arguments pour l'UI.
fn make_preview(tool: &str, args: &serde_json::Value) -> Option<String> {
    match tool {
        "bash" => args
            .get("command")
            .and_then(|v| v.as_str())
            .map(|c| format!("$ {c}")),
        "write_file" | "edit_file" => args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| format!("→ {p}")),
        "read_file" => args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| format!("← {p}")),
        _ => None,
    }
}
