use std::path::Path;

use tracing::info;

use super::db_pool::{DbPool, DbRow, DbValue};
use pylos_core::domain::logging::now_ms;
use pylos_core::domain::mcp_server::{McpServer, McpServerStatus, McpServerType};
use pylos_core::error::PylosError;

#[derive(Clone)]
pub struct McpServerStore {
    pool: DbPool,
}

impl McpServerStore {
    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;
        pool.execute(
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
            vec![],
        )
        .await?;
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

    pub async fn from_pool(pool: DbPool) -> Result<Self, sqlx::Error> {
        let store = Self { pool };
        store.pool.run_migrations().await?;
        Ok(store)
    }

    pub async fn create(&self, server: &McpServer) -> Result<McpServer, PylosError> {
        let server_type_str = server.server_type.to_string();
        let status_str = server.status.to_string();
        let env_str = server.env_vars.as_ref().map(|v| v.to_string());
        let transport_str = server
            .transport_config
            .as_ref()
            .map(|tc| serde_json::to_string(tc).unwrap_or_default());

        self.pool
            .execute(
                "INSERT INTO mcp_servers (id, name, server_type, status, target_url, container_image, env_vars, virtual_key_id, team_id, transport_config, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                vec![
                    DbValue::Text(server.id.clone()),
                    DbValue::Text(server.name.clone()),
                    DbValue::Text(server_type_str),
                    DbValue::Text(status_str),
                    DbValue::OptText(server.target_url.clone()),
                    DbValue::OptText(server.container_image.clone()),
                    DbValue::OptText(env_str),
                    DbValue::OptText(server.virtual_key_id.clone()),
                    DbValue::OptText(server.team_id.clone()),
                    DbValue::OptText(transport_str),
                    DbValue::I64(server.created_at),
                    DbValue::I64(server.updated_at),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to create MCP server: {e}")))?;

        Ok(server.clone())
    }

    pub async fn get(&self, id: &str) -> Result<McpServer, PylosError> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT * FROM mcp_servers WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to get MCP server: {e}")))?
            .ok_or_else(|| PylosError::NotFound(format!("MCP server {id} not found")))?;
        Ok(row_to_mcp_server(&row))
    }

    pub async fn list(&self) -> Result<Vec<McpServer>, PylosError> {
        let rows = self
            .pool
            .fetch_all("SELECT * FROM mcp_servers ORDER BY created_at DESC", vec![])
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to list MCP servers: {e}")))?;
        Ok(rows.iter().map(row_to_mcp_server).collect())
    }

    pub async fn update(&self, server: &McpServer) -> Result<McpServer, PylosError> {
        let server_type_str = server.server_type.to_string();
        let status_str = server.status.to_string();
        let env_str = server.env_vars.as_ref().map(|v| v.to_string());
        let transport_str = server
            .transport_config
            .as_ref()
            .map(|tc| serde_json::to_string(tc).unwrap_or_default());

        self.pool
            .execute(
                "UPDATE mcp_servers SET name = $2, server_type = $3, status = $4, target_url = $5, container_image = $6, env_vars = $7, virtual_key_id = $8, team_id = $9, transport_config = $10, updated_at = $11 WHERE id = $1",
                vec![
                    DbValue::Text(server.id.clone()),
                    DbValue::Text(server.name.clone()),
                    DbValue::Text(server_type_str),
                    DbValue::Text(status_str),
                    DbValue::OptText(server.target_url.clone()),
                    DbValue::OptText(server.container_image.clone()),
                    DbValue::OptText(env_str),
                    DbValue::OptText(server.virtual_key_id.clone()),
                    DbValue::OptText(server.team_id.clone()),
                    DbValue::OptText(transport_str),
                    DbValue::I64(server.updated_at),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to update MCP server: {e}")))?;

        Ok(server.clone())
    }

    pub async fn delete(&self, id: &str) -> Result<(), PylosError> {
        let rows_affected = self
            .pool
            .execute(
                "DELETE FROM mcp_servers WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete MCP server: {e}")))?;

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

        self.pool
            .execute(
                "UPDATE mcp_servers SET status = $1, updated_at = $2 WHERE id = $3",
                vec![
                    DbValue::Text(status_str),
                    DbValue::I64(now),
                    DbValue::Text(id.to_string()),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to set MCP server status: {e}")))?;

        self.get(id).await
    }

    pub async fn find_by_virtual_key(
        &self,
        virtual_key_id: &str,
    ) -> Result<Vec<McpServer>, PylosError> {
        let rows = self
            .pool
            .fetch_all(
                "SELECT * FROM mcp_servers WHERE virtual_key_id = $1 AND status = 'active'",
                vec![DbValue::Text(virtual_key_id.to_string())],
            )
            .await
            .map_err(|e| {
                PylosError::Internal(format!("Failed to find MCP servers by virtual key: {e}"))
            })?;
        Ok(rows.iter().map(row_to_mcp_server).collect())
    }

    pub async fn find_by_team(&self, team_id: &str) -> Result<Vec<McpServer>, PylosError> {
        let rows = self
            .pool
            .fetch_all(
                "SELECT * FROM mcp_servers WHERE team_id = $1 AND status = 'active'",
                vec![DbValue::Text(team_id.to_string())],
            )
            .await
            .map_err(|e| {
                PylosError::Internal(format!("Failed to find MCP servers by team: {e}"))
            })?;
        Ok(rows.iter().map(row_to_mcp_server).collect())
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

fn row_to_mcp_server(row: &DbRow) -> McpServer {
    let env_vars = row
        .get_opt_str("env_vars")
        .and_then(|s| serde_json::from_str(&s).ok());
    let transport_config = row
        .get_opt_str("transport_config")
        .and_then(|s| serde_json::from_str(&s).ok());

    McpServer {
        id: row.get_str("id"),
        name: row.get_str("name"),
        server_type: parse_server_type(&row.get_str("server_type")),
        status: parse_status(&row.get_str("status")),
        target_url: row.get_opt_str("target_url"),
        container_image: row.get_opt_str("container_image"),
        env_vars,
        virtual_key_id: row.get_opt_str("virtual_key_id"),
        team_id: row.get_opt_str("team_id"),
        transport_config,
        created_at: row.get_i64("created_at"),
        updated_at: row.get_i64("updated_at"),
    }
}
