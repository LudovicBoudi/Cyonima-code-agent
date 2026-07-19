//! Persistance SQLite des sessions et de leurs messages.
//!
//! Schéma (émulé via `CREATE TABLE IF NOT EXISTS`, pas de `sqlx::migrate!`
//! pour rester sur une DB simple sans dossier `migrations/` runtime) :
//!
//! ```sql
//! CREATE TABLE sessions (
//!   id            TEXT PRIMARY KEY,
//!   workspace     TEXT NOT NULL,
//!   model_id      TEXT NOT NULL,
//!   provider_id   TEXT NOT NULL,
//!   created_at    TEXT NOT NULL,  -- RFC3339 ISO 8601
//!   title         TEXT,           -- résumé optionnel court
//!   updated_at    TEXT NOT NULL
//! );
//!
//! CREATE TABLE messages (
//!   id            INTEGER PRIMARY KEY AUTOINCREMENT,
//!   session_id    TEXT NOT NULL,
//!   role          TEXT NOT NULL,  -- system|user|assistant|tool
//!   content       TEXT NOT NULL,
//!   seq           INTEGER NOT NULL, -- ordre d'arrivée dans la session
//!   created_at    TEXT NOT NULL,
//!   FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
//! );
//! CREATE INDEX idx_messages_session ON messages(session_id, seq);
//! ```
//!
//! Le fichier DB est à `~/.cyonima/sessions.db`. Configurable en J9 via la
//! config globale.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::Row;
use tokio::sync::Mutex;

use super::SessionInfo;
use crate::providers::{ChatMessage, ProviderKind, Role};

/// Pool SQLite partagé via `AppState`. À l'usage on wrap les opérations dans
/// le pool Sqlx (peek → execute / fetch) — pas de Mutex nécessaire car Sqlx
/// gère déjà la concurrence via pool.
#[derive(Clone)]
pub struct Persistence {
    pool: SqlitePool,
}

/// Pour les tests : PathBuf temporaire à cleanup manuellement.
pub async fn open_at(path: PathBuf) -> anyhow::Result<Persistence> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool = SqlitePool::connect_with(options).await?;
    let p = Persistence { pool };
    p.migrate().await?;
    Ok(p)
}

/// Ouvre la DB à l'emplacement par défaut `<~/.cyonima>/sessions.db`.
pub async fn open_default() -> anyhow::Result<Persistence> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let p = home.join(".cyonima").join("sessions.db");
    open_at(p).await
}

impl Persistence {
    async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id            TEXT PRIMARY KEY,
                workspace     TEXT NOT NULL,
                model_id      TEXT NOT NULL,
                provider_id   TEXT NOT NULL,
                created_at    TEXT NOT NULL,
                title         TEXT,
                updated_at    TEXT NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id    TEXT NOT NULL,
                role          TEXT NOT NULL,
                content       TEXT NOT NULL,
                seq           INTEGER NOT NULL,
                created_at    TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, seq);
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insère une nouvelle session, ou met à jour `updated_at` si elle existe déjà
    /// (utile pour les forks qui réutilisent le même SessionInfo).
    pub async fn upsert_session(&self, info: &SessionInfo) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, workspace, model_id, provider_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                workspace = excluded.workspace,
                model_id = excluded.model_id,
                provider_id = excluded.provider_id,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&info.id)
        .bind(&info.workspace)
        .bind(&info.model_id)
        .bind(info.provider_id.as_str())
        .bind(info.created_at.to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Met à jour le titre optionnel d'une session (pour affichage UI plus parlant).
    pub async fn set_session_title(&self, id: &str, title: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE sessions SET title = ?, updated_at = ? WHERE id = ?")
            .bind(title)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Liste toutes les sessions, triées par `updated_at DESC`. Utilisé au
    /// démarrage de l'app pour restaurer la sidebar.
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>> {
        let rows = sqlx::query(
            "SELECT id, workspace, model_id, provider_id, created_at FROM sessions ORDER BY datetime(updated_at) DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let id: String = r.try_get("id")?;
            let workspace: String = r.try_get("workspace")?;
            let model_id: String = r.try_get("model_id")?;
            let provider_id_str: String = r.try_get("provider_id")?;
            let created_at_str: String = r.try_get("created_at")?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let provider_id = match provider_id_str.as_str() {
                "llama_cpp" => ProviderKind::LlamaCpp,
                "ollama" => ProviderKind::Ollama,
                "openai" => ProviderKind::OpenAi,
                "anthropic" => ProviderKind::Anthropic,
                "gemini" => ProviderKind::Gemini,
                "openai_compat" => ProviderKind::OpenAiCompat,
                _ => ProviderKind::Ollama,
            };
            out.push(SessionInfo {
                id,
                workspace,
                model_id,
                provider_id,
                created_at,
            });
        }
        Ok(out)
    }

    /// Liste le titre (optionnel) d'une session — pour la sidebar plus parlante.
    pub async fn session_title(&self, id: &str) -> anyhow::Result<Option<String>> {
        let row = sqlx::query("SELECT title FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => {
                let t: Option<String> = r.try_get("title")?;
                Ok(t)
            }
            None => Ok(None),
        }
    }

    /// Supprime une session et tous ses messages (CASCADE).
    pub async fn delete_session(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Ajoute un message à la session. `seq` est dérivé du count actuel.
    pub async fn append_message(&self, session_id: &str, msg: &ChatMessage) -> anyhow::Result<()> {
        // Calcule seq = count courant + 1.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE session_id = ?")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;
        let seq = count + 1;
        sqlx::query(
            r#"
            INSERT INTO messages (session_id, role, content, seq, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(session_id)
        .bind(msg.role.as_str())
        .bind(&msg.content)
        .bind(seq)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        // Touch updated_at sur la session.
        sqlx::query("UPDATE sessions SET updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Récupère tous les messages d'une session dans l'ordre.
    pub async fn load_messages(&self, session_id: &str) -> anyhow::Result<Vec<ChatMessage>> {
        let rows =
            sqlx::query("SELECT role, content FROM messages WHERE session_id = ? ORDER BY seq ASC")
                .bind(session_id)
                .fetch_all(&self.pool)
                .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let role_str: String = r.try_get("role")?;
            let content: String = r.try_get("content")?;
            let role = match role_str.as_str() {
                "system" => Role::System,
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "tool" => Role::Tool,
                _ => Role::User,
            };
            out.push(ChatMessage { role, content });
        }
        Ok(out)
    }

    /// Supprime tous les messages d'une session (utile pour réinitialiser
    /// une session vide tout en gardant la métadonnée).
    pub async fn clear_messages(&self, session_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM messages WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Helper : aucune instance n'utilise un Mutex global, mais on garde le
/// typedef pour les signatures futures de SessionManager qui l'utiliseraient.
#[allow(dead_code)]
pub type SharedPersistence = Arc<Mutex<Persistence>>;

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn info() -> SessionInfo {
        SessionInfo::new("/tmp/ws", "test-model", ProviderKind::Ollama)
    }

    #[tokio::test]
    async fn upsert_and_list_sessions() {
        let path = std::env::temp_dir().join(format!("cyonima-sess-{}.db", Uuid::new_v4()));
        let p = open_at(path.clone()).await.unwrap();

        let s = info();
        p.upsert_session(&s).await.unwrap();
        let listed = p.list_sessions().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, s.id);

        // Un 2e
        let s2 = SessionInfo::new("/tmp/ws2", "m2", ProviderKind::LlamaCpp);
        p.upsert_session(&s2).await.unwrap();
        let listed = p.list_sessions().await.unwrap();
        assert_eq!(listed.len(), 2);
        tokio::fs::remove_file(&path).await.ok();
    }

    #[tokio::test]
    async fn append_and_load_messages() {
        let path = std::env::temp_dir().join(format!("cyonima-sess-{}.db", Uuid::new_v4()));
        let p = open_at(path.clone()).await.unwrap();
        let s = info();
        p.upsert_session(&s).await.unwrap();

        p.append_message(
            &s.id,
            &ChatMessage {
                role: Role::User,
                content: "hello".into(),
            },
        )
        .await
        .unwrap();
        p.append_message(
            &s.id,
            &ChatMessage {
                role: Role::Assistant,
                content: "hi!".into(),
            },
        )
        .await
        .unwrap();

        let msgs = p.load_messages(&s.id).await.unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "hello");
        assert_eq!(msgs[1].content, "hi!");
        assert_eq!(msgs[0].role, Role::User);
        assert_eq!(msgs[1].role, Role::Assistant);

        tokio::fs::remove_file(&path).await.ok();
    }

    #[tokio::test]
    async fn delete_session_cascades_messages() {
        let path = std::env::temp_dir().join(format!("cyonima-sess-{}.db", Uuid::new_v4()));
        let p = open_at(path.clone()).await.unwrap();
        let s = info();
        p.upsert_session(&s).await.unwrap();
        p.append_message(
            &s.id,
            &ChatMessage {
                role: Role::User,
                content: "hi".into(),
            },
        )
        .await
        .unwrap();
        p.delete_session(&s.id).await.unwrap();
        let listed = p.list_sessions().await.unwrap();
        assert!(listed.is_empty());
        let msgs = p.load_messages(&s.id).await.unwrap();
        assert!(msgs.is_empty());
        tokio::fs::remove_file(&path).await.ok();
    }

    #[tokio::test]
    async fn set_and_get_title() {
        let path = std::env::temp_dir().join(format!("cyonima-sess-{}.db", Uuid::new_v4()));
        let p = open_at(path.clone()).await.unwrap();
        let s = info();
        p.upsert_session(&s).await.unwrap();
        assert_eq!(p.session_title(&s.id).await.unwrap(), None);
        p.set_session_title(&s.id, "Тest Titre").await.unwrap();
        assert_eq!(
            p.session_title(&s.id).await.unwrap().as_deref(),
            Some("Тest Titre")
        );
        tokio::fs::remove_file(&path).await.ok();
    }
}
