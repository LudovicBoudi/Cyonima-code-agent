//! Gateway de permissions — point de passage unique pour tous les outils.
//!
//! Politique : `auto | ask | deny`. Chargeable depuis `config/`. Les clés
//! manquantes retombent sur les defaults prudents (cf `default_policy`).
//!
//! Le flux UI concret est câblé en J3 : le gateway émet l'event Tauri
//! `permission:request` et bloque le tool jusqu'à la commande IPC
//! `permission_respond`.

use serde::{Deserialize, Serialize};

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

/// Defaults prudents : l'écriture et le shell demandent confirmation à chaque fois.
pub fn default_policy(tool: &str) -> Policy {
    match tool {
        "read_file" | "glob" | "grep" => Policy::Auto,
        "write_file" | "edit_file" | "bash" => Policy::Ask,
        _ => Policy::Ask,
    }
}
