use std::path::Path;

use tracing::info;

use super::db_pool::{DbPool, DbRow, DbValue};
use pylos_core::domain::search_tool::SearchToolConfig;
use pylos_core::error::PylosError;

#[derive(Clone)]
pub struct SearchToolStore {
    pool: DbPool,
}

impl SearchToolStore {
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_sqlite(db_path, "search_tool_store", 4).await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!(path = %db_path.display(), "Search tool store opened (SQLite)");
        Ok(store)
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;
        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS search_tools (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                tool_type   TEXT NOT NULL,
                config      TEXT NOT NULL DEFAULT '{}',
                is_active   INTEGER NOT NULL DEFAULT 1,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL
            )"#,
            vec![],
        )
        .await?;
        Ok(Self { pool })
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_postgres(database_url, "search_tool_store").await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!("Search tool store opened (PostgreSQL)");
        Ok(store)
    }

    pub async fn from_pool(pool: DbPool) -> Result<Self, sqlx::Error> {
        let store = Self { pool };
        store.pool.run_migrations().await?;
        Ok(store)
    }

    pub async fn list_search_tools(&self) -> Result<Vec<SearchToolConfig>, PylosError> {
        let rows = self
            .pool
            .fetch_all("SELECT * FROM search_tools ORDER BY name", vec![])
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to list search tools: {}", e)))?;
        Ok(rows.iter().map(row_to_search_tool).collect())
    }

    pub async fn get_search_tool(&self, id: &str) -> Result<Option<SearchToolConfig>, PylosError> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT * FROM search_tools WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to get search tool: {}", e)))?;
        Ok(row.as_ref().map(row_to_search_tool))
    }

    pub async fn upsert_search_tool(&self, st: &SearchToolConfig) -> Result<(), PylosError> {
        self.pool
            .execute(
                r#"INSERT INTO search_tools (id, name, description, tool_type, config, is_active, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    tool_type = excluded.tool_type,
                    config = excluded.config,
                    is_active = excluded.is_active,
                    updated_at = excluded.updated_at"#,
                vec![
                    DbValue::Text(st.id.clone()),
                    DbValue::Text(st.name.clone()),
                    DbValue::OptText(st.description.clone()),
                    DbValue::Text(st.tool_type.clone()),
                    DbValue::Json(st.config.clone()),
                    DbValue::Bool(st.is_active),
                    DbValue::I64(st.created_at),
                    DbValue::I64(st.updated_at),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to upsert search tool: {}", e)))?;
        Ok(())
    }

    pub async fn delete_search_tool(&self, id: &str) -> Result<bool, PylosError> {
        let rows = self
            .pool
            .execute(
                "DELETE FROM search_tools WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete search tool: {}", e)))?;
        Ok(rows > 0)
    }
}

fn row_to_search_tool(row: &DbRow) -> SearchToolConfig {
    SearchToolConfig {
        id: row.get_str("id"),
        name: row.get_str("name"),
        description: row.get_opt_str("description"),
        tool_type: row.get_str("tool_type"),
        config: row
            .get_json("config")
            .unwrap_or_else(|| serde_json::Value::Object(Default::default())),
        is_active: row.get_bool("is_active"),
        created_at: row.get_i64("created_at"),
        updated_at: row.get_i64("updated_at"),
    }
}
