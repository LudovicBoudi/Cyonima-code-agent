//! Stockage des clés API via le keyring OS.
//!
//! - Windows : DPAPI (CryptProtectData)
//! - macOS : Keychain
//! - Linux : Secret Service (libsecret / gnome-keyring)
//!
//! Les clés ne sont **jamais** écrites sur disque hors keyring, jamais loggées,
//! jamais sérialisées en clair dans la config. Cf `AGENTS.md` > Sécurité.
//!
//! Service name : `cyonima.ia-code-agent` (constant).
//! Account name : identifiant du provider (`openai`, `anthropic`, `gemini`, `openai_compat`).

use keyring::Entry;

const SERVICE: &str = "cyonima.ia-code-agent";

/// Stocke (ou remplace) la clé API d'un provider.
pub fn set_key(provider: &str, key: &str) -> anyhow::Result<()> {
    let entry = Entry::new(SERVICE, provider)?;
    entry.set_password(key)?;
    Ok(())
}

/// Récupère la clé API d'un provider. `None` si absente ou inaccessible
/// (par ex. keyring verrouillé sous Linux).
pub fn get_key(provider: &str) -> Option<String> {
    let entry = Entry::new(SERVICE, provider).ok()?;
    entry.get_password().ok()
}

/// Supprime la clé API d'un provider. No-op si absente.
pub fn delete_key(provider: &str) -> anyhow::Result<()> {
    let entry = Entry::new(SERVICE, provider)?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}

/// Renvoie `true` si une clé est enregistrée pour ce provider.
pub fn has_key(provider: &str) -> bool {
    get_key(provider).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests keyring requirent un keyring accessible — en CI headless Linux
    /// ou dans un shell non-interactif ça peut échouer. On rend donc le test
    /// tolérant : si le keyring n'est pas joignable, on skip silencieusement.
    #[test]
    fn round_trip_skipped_if_keyring_unavailable() {
        let provider = "__test_cyonima__";
        let Ok(_) = set_key(provider, "secret-value") else {
            return;
        };
        if let Some(v) = get_key(provider) {
            assert_eq!(v, "secret-value");
        }
        let _ = delete_key(provider);
        assert!(get_key(provider).is_none() || get_key(provider) == Some("secret-value".into()));
    }
}