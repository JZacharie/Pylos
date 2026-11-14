use std::path::Path;
use tracing::info;

use super::db_pool::{DbPool, DbValue};
use pylos_core::domain::system_prompt::SystemPrompt;
use pylos_core::error::PylosError;

#[derive(Clone)]
pub struct SystemPromptStore {
    pool: DbPool,
}

impl SystemPromptStore {
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_sqlite(db_path, "system_prompt_store", 4).await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!(path = %db_path.display(), "System prompt store opened (SQLite)");
        Ok(store)
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_postgres(database_url, "system_prompt_store").await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!("System prompt store opened (PostgreSQL)");
        Ok(store)
    }

    pub async fn from_pool(pool: DbPool) -> Result<Self, sqlx::Error> {
        let store = Self { pool };
        store.pool.run_migrations().await?;
        Ok(store)
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;
        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS system_prompts (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                prompt      TEXT NOT NULL,
                created_at  TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
            vec![],
        )
        .await?;
        Ok(Self { pool })
    }

    pub async fn list_prompts(&self) -> Result<Vec<SystemPrompt>, PylosError> {
        let rows = self
            .pool
            .fetch_all("SELECT id, name, prompt FROM system_prompts ORDER BY id", vec![])
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to list system prompts: {}", e)))?;

        Ok(rows
            .iter()
            .map(|r| SystemPrompt {
                id: r.get_str("id"),
                name: r.get_str("name"),
                prompt: r.get_str("prompt"),
            })
            .collect())
    }

    pub async fn get_prompt_by_id(&self, id: &str) -> Result<Option<SystemPrompt>, PylosError> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT id, name, prompt FROM system_prompts WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to get system prompt: {}", e)))?;

        Ok(row.map(|r| SystemPrompt {
            id: r.get_str("id"),
            name: r.get_str("name"),
            prompt: r.get_str("prompt"),
        }))
    }

    pub async fn upsert_prompt(&self, sp: &SystemPrompt) -> Result<(), PylosError> {
        self.pool
            .execute(
                r#"
                INSERT INTO system_prompts (id, name, prompt)
                VALUES ($1, $2, $3)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    prompt = excluded.prompt
                "#,
                vec![
                    DbValue::Text(sp.id.clone()),
                    DbValue::Text(sp.name.clone()),
                    DbValue::Text(sp.prompt.clone()),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to upsert system prompt: {}", e)))?;
        Ok(())
    }

    pub async fn delete_prompt(&self, id: &str) -> Result<bool, PylosError> {
        let rows_affected = self
            .pool
            .execute(
                "DELETE FROM system_prompts WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete system prompt: {}", e)))?;
        Ok(rows_affected > 0)
    }
}
