use std::path::Path;

use sqlx::Row;
use tracing::info;

use crate::db_pool::DbPool;
use crate::log_store::now_ms;
use pylos_core::domain::mcp_server::{McpServer, McpServerStatus, McpServerType};
use pylos_core::error::PylosError;

#[derive(Clone)]
pub struct McpServerStore {
    pool: DbPool,
}

impl McpServerStore {
    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;
        if let Some(p) = pool.as_sqlite() {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS mcp_servers (
                    id                TEXT PRIMARY KEY,
                    name              TEXT NOT NULL,
                    server_type       TEXT NOT NULL DEFAULT 'python',
                    status            TEXT NOT NULL DEFAULT 'inactive',
                    target_url        TEXT,
                    container_image   TEXT,
                    env_vars          TEXT,
                    virtual_key_id    TEXT,
                    team_id           TEXT,
                    transport_config  TEXT,
                    created_at        INTEGER NOT NULL,
                    updated_at        INTEGER NOT NULL
                )"#,
            )
            .execute(p)
            .await?;
        }
        let store = Self { pool };
        info!("MCP server store opened (in-memory)");
        Ok(store)
    }

    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_sqlite(db_path, "mcp_server_store", 2).await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!(path = %db_path.display(), "MCP server store opened (SQLite)");
        Ok(store)
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_postgres(database_url, "mcp_server_store").await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!("MCP server store opened (PostgreSQL)");
        Ok(store)
    }

    pub async fn create(&self, server: &McpServer) -> Result<McpServer, PylosError> {
        let server_type_str = server.server_type.to_string();
        let status_str = server.status.to_string();
        let env_str = server.env_vars.as_ref().map(|v| v.to_string());
        let tc_str = server
            .transport_config
            .as_ref()
            .and_then(|tc| serde_json::to_string(tc).ok());

        match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO mcp_servers (id, name, server_type, status, target_url, container_image, env_vars, virtual_key_id, team_id, transport_config, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&server.id)
                .bind(&server.name)
                .bind(&server_type_str)
                .bind(&status_str)
                .bind(&server.target_url)
                .bind(&server.container_image)
                .bind(&env_str)
                .bind(&server.virtual_key_id)
                .bind(&server.team_id)
                .bind(&tc_str)
                .bind(server.created_at)
                .bind(server.updated_at)
                .execute(pool)
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to create MCP server: {e}")))?;
            }
            DbPool::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO mcp_servers (id, name, server_type, status, target_url, container_image, env_vars, virtual_key_id, team_id, transport_config, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                )
                .bind(&server.id)
                .bind(&server.name)
                .bind(&server_type_str)
                .bind(&status_str)
                .bind(&server.target_url)
                .bind(&server.container_image)
                .bind(&env_str)
                .bind(&server.virtual_key_id)
                .bind(&server.team_id)
                .bind(&tc_str)
                .bind(server.created_at)
                .bind(server.updated_at)
                .execute(pool)
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to create MCP server: {e}")))?;
            }
        }

        Ok(server.clone())
    }

    pub async fn get(&self, id: &str) -> Result<McpServer, PylosError> {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let row = sqlx::query("SELECT * FROM mcp_servers WHERE id = ?")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| PylosError::Internal(format!("Failed to get MCP server: {e}")))?
                    .ok_or_else(|| PylosError::NotFound(format!("MCP server {id} not found")))?;
                row_to_mcp_server_sqlite(&row)
            }
            DbPool::Postgres(pool) => {
                let row = sqlx::query("SELECT * FROM mcp_servers WHERE id = $1")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| PylosError::Internal(format!("Failed to get MCP server: {e}")))?
                    .ok_or_else(|| PylosError::NotFound(format!("MCP server {id} not found")))?;
                row_to_mcp_server_pg(&row)
            }
        }
    }

    pub async fn list(&self) -> Result<Vec<McpServer>, PylosError> {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let rows = sqlx::query("SELECT * FROM mcp_servers ORDER BY created_at DESC")
                    .fetch_all(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to list MCP servers: {e}"))
                    })?;
                rows.iter().map(row_to_mcp_server_sqlite).collect()
            }
            DbPool::Postgres(pool) => {
                let rows: Vec<sqlx::postgres::PgRow> =
                    sqlx::query("SELECT * FROM mcp_servers ORDER BY created_at DESC")
                        .fetch_all(pool)
                        .await
                        .map_err(|e| {
                            PylosError::Internal(format!("Failed to list MCP servers: {e}"))
                        })?;
                rows.iter().map(row_to_mcp_server_pg).collect()
            }
        }
    }

    pub async fn update(&self, server: &McpServer) -> Result<McpServer, PylosError> {
        let server_type_str = server.server_type.to_string();
        let status_str = server.status.to_string();
        let env_str = server.env_vars.as_ref().map(|v| v.to_string());
        let tc_str = server
            .transport_config
            .as_ref()
            .and_then(|tc| serde_json::to_string(tc).ok());

        match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE mcp_servers SET name = ?, server_type = ?, status = ?, target_url = ?, container_image = ?, env_vars = ?, virtual_key_id = ?, team_id = ?, transport_config = ?, updated_at = ? WHERE id = ?",
                )
                .bind(&server.name)
                .bind(&server_type_str)
                .bind(&status_str)
                .bind(&server.target_url)
                .bind(&server.container_image)
                .bind(&env_str)
                .bind(&server.virtual_key_id)
                .bind(&server.team_id)
                .bind(&tc_str)
                .bind(server.updated_at)
                .bind(&server.id)
                .execute(pool)
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to update MCP server: {e}")))?;
            }
            DbPool::Postgres(pool) => {
                sqlx::query(
                    "UPDATE mcp_servers SET name = $2, server_type = $3, status = $4, target_url = $5, container_image = $6, env_vars = $7, virtual_key_id = $8, team_id = $9, transport_config = $10, updated_at = $11 WHERE id = $1",
                )
                .bind(&server.id)
                .bind(&server.name)
                .bind(&server_type_str)
                .bind(&status_str)
                .bind(&server.target_url)
                .bind(&server.container_image)
                .bind(&env_str)
                .bind(&server.virtual_key_id)
                .bind(&server.team_id)
                .bind(&tc_str)
                .bind(server.updated_at)
                .execute(pool)
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to update MCP server: {e}")))?;
            }
        }

        Ok(server.clone())
    }

    pub async fn delete(&self, id: &str) -> Result<(), PylosError> {
        let rows_affected = match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query("DELETE FROM mcp_servers WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to delete MCP server: {e}")))?
                .rows_affected(),
            DbPool::Postgres(pool) => sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to delete MCP server: {e}")))?
                .rows_affected(),
        };

        if rows_affected == 0 {
            return Err(PylosError::NotFound(format!("MCP server {id} not found")));
        }
        Ok(())
    }

    pub async fn set_status(
        &self,
        id: &str,
        status: &McpServerStatus,
    ) -> Result<McpServer, PylosError> {
        let now = now_ms();
        let status_str = status.to_string();

        match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query("UPDATE mcp_servers SET status = ?, updated_at = ? WHERE id = ?")
                    .bind(&status_str)
                    .bind(now)
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to set MCP server status: {e}"))
                    })?;
            }
            DbPool::Postgres(pool) => {
                sqlx::query("UPDATE mcp_servers SET status = $1, updated_at = $2 WHERE id = $3")
                    .bind(&status_str)
                    .bind(now)
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to set MCP server status: {e}"))
                    })?;
            }
        }

        self.get(id).await
    }

    pub async fn find_by_virtual_key(
        &self,
        virtual_key_id: &str,
    ) -> Result<Vec<McpServer>, PylosError> {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let rows = sqlx::query(
                    "SELECT * FROM mcp_servers WHERE virtual_key_id = ? AND status = 'active'",
                )
                .bind(virtual_key_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to find MCP servers by virtual key: {e}"))
                })?;
                rows.iter().map(row_to_mcp_server_sqlite).collect()
            }
            DbPool::Postgres(pool) => {
                let rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
                    "SELECT * FROM mcp_servers WHERE virtual_key_id = $1 AND status = 'active'",
                )
                .bind(virtual_key_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to find MCP servers by virtual key: {e}"))
                })?;
                rows.iter().map(row_to_mcp_server_pg).collect()
            }
        }
    }

    pub async fn find_by_team(&self, team_id: &str) -> Result<Vec<McpServer>, PylosError> {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let rows = sqlx::query(
                    "SELECT * FROM mcp_servers WHERE team_id = ? AND status = 'active'",
                )
                .bind(team_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to find MCP servers by team: {e}"))
                })?;
                rows.iter().map(row_to_mcp_server_sqlite).collect()
            }
            DbPool::Postgres(pool) => {
                let rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
                    "SELECT * FROM mcp_servers WHERE team_id = $1 AND status = 'active'",
                )
                .bind(team_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to find MCP servers by team: {e}"))
                })?;
                rows.iter().map(row_to_mcp_server_pg).collect()
            }
        }
    }
}

fn parse_server_type(s: &str) -> McpServerType {
    match s {
        "python" => McpServerType::Python,
        "node" => McpServerType::Node,
        _ => McpServerType::Custom(s.to_string()),
    }
}

fn parse_status(s: &str) -> McpServerStatus {
    match s {
        "active" => McpServerStatus::Active,
        "inactive" => McpServerStatus::Inactive,
        "error" => McpServerStatus::Error,
        _ => McpServerStatus::Inactive,
    }
}

fn row_to_mcp_server_sqlite(row: &sqlx::sqlite::SqliteRow) -> Result<McpServer, PylosError> {
    let env_raw: Option<String> = row.try_get("env_vars").ok().flatten();
    let env_vars = env_raw.and_then(|s| serde_json::from_str(&s).ok());
    let tc_raw: Option<String> = row.try_get("transport_config").ok().flatten();
    let transport_config = tc_raw.and_then(|s| serde_json::from_str(&s).ok());

    Ok(McpServer {
        id: row
            .try_get("id")
            .map_err(|e| PylosError::Internal(format!("Bad id: {e}")))?,
        name: row
            .try_get("name")
            .map_err(|e| PylosError::Internal(format!("Bad name: {e}")))?,
        server_type: row
            .try_get::<String, _>("server_type")
            .map(|s| parse_server_type(&s))
            .map_err(|e| PylosError::Internal(format!("Bad server_type: {e}")))?,
        status: row
            .try_get::<String, _>("status")
            .map(|s| parse_status(&s))
            .map_err(|e| PylosError::Internal(format!("Bad status: {e}")))?,
        target_url: row.try_get("target_url").ok().flatten(),
        container_image: row.try_get("container_image").ok().flatten(),
        env_vars,
        virtual_key_id: row.try_get("virtual_key_id").ok().flatten(),
        team_id: row.try_get("team_id").ok().flatten(),
        transport_config,
        created_at: row
            .try_get("created_at")
            .map_err(|e| PylosError::Internal(format!("Bad created_at: {e}")))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|e| PylosError::Internal(format!("Bad updated_at: {e}")))?,
    })
}

fn row_to_mcp_server_pg(row: &sqlx::postgres::PgRow) -> Result<McpServer, PylosError> {
    let env_raw: Option<String> = row.try_get("env_vars").ok().flatten();
    let env_vars = env_raw.and_then(|s| serde_json::from_str(&s).ok());
    let tc_raw: Option<String> = row.try_get("transport_config").ok().flatten();
    let transport_config = tc_raw.and_then(|s| serde_json::from_str(&s).ok());

    Ok(McpServer {
        id: row
            .try_get("id")
            .map_err(|e| PylosError::Internal(format!("Bad id: {e}")))?,
        name: row
            .try_get("name")
            .map_err(|e| PylosError::Internal(format!("Bad name: {e}")))?,
        server_type: row
            .try_get::<String, _>("server_type")
            .map(|s| parse_server_type(&s))
            .map_err(|e| PylosError::Internal(format!("Bad server_type: {e}")))?,
        status: row
            .try_get::<String, _>("status")
            .map(|s| parse_status(&s))
            .map_err(|e| PylosError::Internal(format!("Bad status: {e}")))?,
        target_url: row.try_get("target_url").ok().flatten(),
        container_image: row.try_get("container_image").ok().flatten(),
        env_vars,
        virtual_key_id: row.try_get("virtual_key_id").ok().flatten(),
        team_id: row.try_get("team_id").ok().flatten(),
        transport_config,
        created_at: row
            .try_get("created_at")
            .map_err(|e| PylosError::Internal(format!("Bad created_at: {e}")))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|e| PylosError::Internal(format!("Bad updated_at: {e}")))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::mcp_server::McpTransportConfig;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_mcp_server_store_transport_config_crud() {
        let store = McpServerStore::in_memory().await.unwrap();

        // 1. Create with Stdio transport
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret".to_string());
        let stdio_cfg = McpTransportConfig::Stdio {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-postgres".to_string(),
            ],
            env: Some(env),
            cwd: Some("/app".to_string()),
        };

        let server = McpServer {
            id: "mcp-test-1".to_string(),
            name: "Postgres MCP".to_string(),
            server_type: McpServerType::Node,
            status: McpServerStatus::Inactive,
            target_url: None,
            container_image: None,
            env_vars: None,
            virtual_key_id: Some("vk-1".to_string()),
            team_id: Some("team-eng".to_string()),
            transport_config: Some(stdio_cfg.clone()),
            created_at: 1000,
            updated_at: 1000,
        };

        let created = store.create(&server).await.unwrap();
        assert_eq!(created.id, "mcp-test-1");

        // 2. Get and verify transport_config
        let fetched = store.get("mcp-test-1").await.unwrap();
        assert_eq!(fetched.transport_config, Some(stdio_cfg));
        assert_eq!(fetched.name, "Postgres MCP");

        // 3. Update to Http transport
        let http_cfg = McpTransportConfig::Http {
            url: "http://localhost:9000/mcp".to_string(),
            headers: None,
        };
        let mut updated = fetched;
        updated.transport_config = Some(http_cfg.clone());
        updated.updated_at = 2000;

        let saved = store.update(&updated).await.unwrap();
        assert_eq!(saved.transport_config, Some(http_cfg.clone()));

        let fetched_updated = store.get("mcp-test-1").await.unwrap();
        assert_eq!(fetched_updated.transport_config, Some(http_cfg));

        // 4. Create server with None transport_config (backward compatibility)
        let legacy_server = McpServer {
            id: "mcp-legacy".to_string(),
            name: "Legacy Proxy".to_string(),
            server_type: McpServerType::Python,
            status: McpServerStatus::Active,
            target_url: Some("http://legacy:8000/mcp".to_string()),
            container_image: None,
            env_vars: None,
            virtual_key_id: None,
            team_id: None,
            transport_config: None,
            created_at: 3000,
            updated_at: 3000,
        };
        store.create(&legacy_server).await.unwrap();

        let fetched_legacy = store.get("mcp-legacy").await.unwrap();
        assert_eq!(fetched_legacy.transport_config, None);
        assert_eq!(
            fetched_legacy.effective_transport(),
            Some(McpTransportConfig::Http {
                url: "http://legacy:8000/mcp".to_string(),
                headers: None,
            })
        );

        // 5. List
        let list = store.list().await.unwrap();
        assert_eq!(list.len(), 2);

        // 6. Delete
        store.delete("mcp-test-1").await.unwrap();
        assert!(store.get("mcp-test-1").await.is_err());
    }
}
