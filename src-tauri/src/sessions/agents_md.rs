//! Lecture et injection du AGENTS.md à la racine du workspace.
//!
//! Convention reprise d'Opencode : si l'utilisateur dépose un fichier
//! `AGENTS.md` à la racine de son projet, son contenu est injecté comme
//! premier message `system` du contexte de chaque session Cyonima qui
//! s'ouvre sur ce workspace. Utile pour donner des consignes de style,
//! des règles de sécurité, des notes d'architecture, etc.

use std::path::Path;

/// Charge le AGENTS.md depuis `<workspace>/AGENTS.md`. Renvoie `None` si
/// absent ou illisible (on log, on n'avorte pas — l'absence n'est pas une
/// erreur).
pub fn load(workspace: &Path) -> Option<String> {
    let candidates = [workspace.join("AGENTS.md"), workspace.join("agents.md")];
    for p in &candidates {
        if let Ok(content) = std::fs::read_to_string(p) {
            return Some(content);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_returns_none_when_absent() {
        let tmp = std::env::temp_dir().join(format!("cyonima-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        assert!(load(&tmp).is_none());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn load_returns_content_when_present() {
        let tmp = std::env::temp_dir().join(format!("cyonima-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let mut f = std::fs::File::create(tmp.join("AGENTS.md")).unwrap();
        writeln!(f, "# Hello\nTest AGENTS content.").unwrap();
        let loaded = load(&tmp).unwrap();
        assert!(loaded.contains("Test AGENTS content."));
        std::fs::remove_dir_all(&tmp).ok();
    }
}
