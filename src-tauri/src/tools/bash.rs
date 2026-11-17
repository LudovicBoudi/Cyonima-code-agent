//! Outil `bash` — exécution d'une commande shell après approbation.
//!
//! Politique prudente : `Ask` (cf `permissions::default_policy`). L'utilisateur
//! doit approuver chaque commande avant qu'elle ne s'exécute. Le gateway
//! émet l'event `permission:request` avec la commande en clair ; le frontend
//! la montre, l'utilisateur clique Allow/Deny.
//!
//! Système hôte : sur Windows on wrapper via `cmd /C`, ailleurs `/bin/sh -c`.
//! On n'utilise jamais PowerShell pour rester portable et prévisible côté UI.
//!
//! Limites V1 : pas de streaming stdout, on attend la fin de la commande
//! (timeout 30s), puis on renvoie stdout+stderr tronqués.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;

use super::{truncate_for_llm, Tool, ToolOutput, ToolSpec};

pub struct Bash;

impl Bash {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Bash {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for Bash {
    fn name(&self) -> &str {
        "bash"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "bash".into(),
            description: "Exécute une commande shell dans le workspace. La commande s'exécute avec le workspace comme répertoire courant. Demande une approbation utilisateur explicite.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Commande shell complète" },
                    "timeout": { "type": "integer", "description": "Timeout en secondes (défaut 30, max 300)" }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value, workspace: &Path) -> ToolOutput {
        let Some(cmd) = args.get("command").and_then(|v| v.as_str()) else {
            return ToolOutput::err("bash", "argument `command` manquant");
        };
        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .map(|t| t.clamp(1, 300))
            .unwrap_or(30);

        // Exécution asynchrone (tokio::process) : le timeout est réellement
        // appliqué. `kill_on_drop(true)` garantit que le process est tué si le
        // future `wait_with_output` est abandonné au timeout.
        let mut builder = if cfg!(target_os = "windows") {
            let mut b = Command::new("cmd");
            b.args(["/C", cmd]);
            b
        } else {
            let mut b = Command::new("/bin/sh");
            b.args(["-c", cmd]);
            b
        };
        builder
            .current_dir(workspace)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let child = match builder.spawn() {
            Ok(c) => c,
            Err(e) => return ToolOutput::err("bash", format!("échec spawn: {e}")),
        };

        match tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output())
            .await
        {
            Ok(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = if stdout.is_empty() {
                    stderr.clone()
                } else if stderr.is_empty() {
                    stdout.clone()
                } else {
                    format!("STDOUT:\n{stdout}\n\nSTDERR:\n{stderr}")
                };
                let exit = out.status.code().unwrap_or(-1);
                let body = format!("exit_code={exit}\n{combined}");
                ToolOutput::ok("bash", truncate_for_llm(&body, 8_192))
            }
            Ok(Err(e)) => ToolOutput::err("bash", format!("échec wait: {e}")),
            Err(_) => ToolOutput::err(
                "bash",
                format!(
                    "commande interrompue après {timeout_secs}s (timeout dépassé, process tué)"
                ),
            ),
        }
    }
}
