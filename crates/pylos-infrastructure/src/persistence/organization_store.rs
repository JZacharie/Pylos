use std::path::Path;

use async_trait::async_trait;
use tracing::info;

use super::db_pool::{DbPool, DbRow, DbValue};
use pylos_core::domain::organization::{
    AccessGroup, InternalUser, Organization, Policy, Team, ToolPolicy,
};
use pylos_core::domain::repositories::OrganizationRepository;
use pylos_core::error::PylosError;

#[derive(Clone)]
pub struct OrganizationStore {
    pool: DbPool,
}

impl OrganizationStore {
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_sqlite(db_path, "organization_store", 4).await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!(path = %db_path.display(), "Organization store opened (SQLite)");
        Ok(store)
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_postgres(database_url, "organization_store").await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!("Organization store opened (PostgreSQL)");
        Ok(store)
    }

    pub async fn from_pool(pool: DbPool) -> Result<Self, sqlx::Error> {
        let store = Self { pool };
        store.pool.run_migrations().await?;
        Ok(store)
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;
        let ddls = [
            r#"CREATE TABLE IF NOT EXISTS organizations (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                is_active   INTEGER NOT NULL DEFAULT 1,
                tags        TEXT NOT NULL DEFAULT '[]',
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS teams (
                id              TEXT PRIMARY KEY,
                organization_id TEXT NOT NULL,
                name            TEXT NOT NULL,
                description     TEXT,
                is_active       INTEGER NOT NULL DEFAULT 1,
                tags            TEXT NOT NULL DEFAULT '[]',
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS internal_users (
                id              TEXT PRIMARY KEY,
                email           TEXT NOT NULL UNIQUE,
                name            TEXT NOT NULL,
                role            TEXT NOT NULL DEFAULT 'member',
                usr_group       TEXT NOT NULL DEFAULT 'default',
                organization_id TEXT,
                team_ids        TEXT NOT NULL DEFAULT '[]',
                is_active       INTEGER NOT NULL DEFAULT 1,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS access_groups (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                description     TEXT,
                organization_id TEXT,
                team_ids        TEXT NOT NULL DEFAULT '[]',
                user_ids        TEXT NOT NULL DEFAULT '[]',
                model_ids       TEXT NOT NULL DEFAULT '[]',
                provider_ids    TEXT NOT NULL DEFAULT '[]',
                is_active       INTEGER NOT NULL DEFAULT 1,
                tags            TEXT NOT NULL DEFAULT '[]',
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS policies (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                policy_type TEXT NOT NULL,
                config      TEXT NOT NULL DEFAULT '{}',
                is_active   INTEGER NOT NULL DEFAULT 1,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS tool_policies (
                id                   TEXT PRIMARY KEY,
                name                 TEXT NOT NULL,
                description          TEXT,
                tool_type            TEXT NOT NULL,
                allowed_models       TEXT NOT NULL DEFAULT '[]',
                allowed_providers    TEXT NOT NULL DEFAULT '[]',
                max_tokens_per_call  INTEGER,
                max_calls_per_minute INTEGER,
                is_active            INTEGER NOT NULL DEFAULT 1,
                created_at           INTEGER NOT NULL,
                updated_at           INTEGER NOT NULL
            )"#,
        ];
        for ddl in ddls {
            pool.execute(ddl, vec![]).await?;
        }
        Ok(Self { pool })
    }

    // ── Organizations ──────────────────────────────────────────────────────────

    pub async fn list_organizations(
        &self,
        tag: Option<&str>,
    ) -> Result<Vec<Organization>, PylosError> {
        let rows = self
            .pool
            .fetch_all("SELECT * FROM organizations ORDER BY name", vec![])
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to list organizations: {}", e)))?;
        let mut list: Vec<Organization> = rows.iter().map(row_to_org).collect();
        if let Some(t) = tag {
            list.retain(|o| o.tags.iter().any(|x| x == t));
        }
        Ok(list)
    }

    pub async fn get_organization(&self, id: &str) -> Result<Option<Organization>, PylosError> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT * FROM organizations WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to get organization: {}", e)))?;
        Ok(row.as_ref().map(row_to_org))
    }

    pub async fn upsert_organization(&self, org: &Organization) -> Result<(), PylosError> {
        let tags_val = serde_json::to_value(&org.tags).unwrap_or(serde_json::Value::Array(vec![]));
        self.pool
            .execute(
                r#"INSERT INTO organizations (id, name, description, is_active, tags, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    is_active = excluded.is_active,
                    tags = excluded.tags,
                    updated_at = excluded.updated_at"#,
                vec![
                    DbValue::Text(org.id.clone()),
                    DbValue::Text(org.name.clone()),
                    DbValue::OptText(org.description.clone()),
                    DbValue::Bool(org.is_active),
                    DbValue::Json(tags_val),
                    DbValue::I64(org.created_at),
                    DbValue::I64(org.updated_at),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to upsert organization: {}", e)))?;
        Ok(())
    }

    pub async fn delete_organization(&self, id: &str) -> Result<bool, PylosError> {
        let rows = self
            .pool
            .execute(
                "DELETE FROM organizations WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete organization: {}", e)))?;
        Ok(rows > 0)
    }

    // ── Teams ──────────────────────────────────────────────────────────────────

    pub async fn list_teams(
        &self,
        organization_id: Option<&str>,
        tag: Option<&str>,
    ) -> Result<Vec<Team>, PylosError> {
        let rows = if let Some(org_id) = organization_id {
            self.pool
                .fetch_all(
                    "SELECT * FROM teams WHERE organization_id = $1 ORDER BY name",
                    vec![DbValue::Text(org_id.to_string())],
                )
                .await
        } else {
            self.pool
                .fetch_all("SELECT * FROM teams ORDER BY name", vec![])
                .await
        }
        .map_err(|e| PylosError::Internal(format!("Failed to list teams: {}", e)))?;

        let mut list: Vec<Team> = rows.iter().map(row_to_team).collect();
        if let Some(t) = tag {
            list.retain(|team| team.tags.iter().any(|x| x == t));
        }
        Ok(list)
    }

    pub async fn get_team(&self, id: &str) -> Result<Option<Team>, PylosError> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT * FROM teams WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to get team: {}", e)))?;
        Ok(row.as_ref().map(row_to_team))
    }

    pub async fn upsert_team(&self, team: &Team) -> Result<(), PylosError> {
        let tags_val =
            serde_json::to_value(&team.tags).unwrap_or(serde_json::Value::Array(vec![]));
        self.pool
            .execute(
                r#"INSERT INTO teams (id, organization_id, name, description, is_active, tags, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT(id) DO UPDATE SET
                    organization_id = excluded.organization_id,
                    name = excluded.name,
                    description = excluded.description,
                    is_active = excluded.is_active,
                    tags = excluded.tags,
                    updated_at = excluded.updated_at"#,
                vec![
                    DbValue::Text(team.id.clone()),
                    DbValue::Text(team.organization_id.clone()),
                    DbValue::Text(team.name.clone()),
                    DbValue::OptText(team.description.clone()),
                    DbValue::Bool(team.is_active),
                    DbValue::Json(tags_val),
                    DbValue::I64(team.created_at),
                    DbValue::I64(team.updated_at),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to upsert team: {}", e)))?;
        Ok(())
    }

    pub async fn delete_team(&self, id: &str) -> Result<bool, PylosError> {
        let rows = self
            .pool
            .execute(
                "DELETE FROM teams WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete team: {}", e)))?;
        Ok(rows > 0)
    }

    // ── Users ──────────────────────────────────────────────────────────────────

    pub async fn list_users(&self) -> Result<Vec<InternalUser>, PylosError> {
        let rows = self
            .pool
            .fetch_all("SELECT * FROM internal_users ORDER BY name", vec![])
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to list users: {}", e)))?;
        Ok(rows.iter().map(row_to_user).collect())
    }

    pub async fn get_user(&self, id: &str) -> Result<Option<InternalUser>, PylosError> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT * FROM internal_users WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to get user: {}", e)))?;
        Ok(row.as_ref().map(row_to_user))
    }

    pub async fn upsert_user(&self, user: &InternalUser) -> Result<(), PylosError> {
        let team_ids_val =
            serde_json::to_value(&user.team_ids).unwrap_or(serde_json::Value::Array(vec![]));
        self.pool
            .execute(
                r#"INSERT INTO internal_users (id, email, name, role, usr_group, organization_id, team_ids, is_active, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT(id) DO UPDATE SET
                    email = excluded.email,
                    name = excluded.name,
                    role = excluded.role,
                    usr_group = excluded.usr_group,
                    organization_id = excluded.organization_id,
                    team_ids = excluded.team_ids,
                    is_active = excluded.is_active,
                    updated_at = excluded.updated_at"#,
                vec![
                    DbValue::Text(user.id.clone()),
                    DbValue::Text(user.email.clone()),
                    DbValue::Text(user.name.clone()),
                    DbValue::Text(user.role.clone()),
                    DbValue::Text(user.group.clone()),
                    DbValue::OptText(user.organization_id.clone()),
                    DbValue::Json(team_ids_val),
                    DbValue::Bool(user.is_active),
                    DbValue::I64(user.created_at),
                    DbValue::I64(user.updated_at),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to upsert user: {}", e)))?;
        Ok(())
    }

    pub async fn delete_user(&self, id: &str) -> Result<bool, PylosError> {
        let rows = self
            .pool
            .execute(
                "DELETE FROM internal_users WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete user: {}", e)))?;
        Ok(rows > 0)
    }

    // ── Access Groups ──────────────────────────────────────────────────────────

    pub async fn list_access_groups(
        &self,
        tag: Option<&str>,
    ) -> Result<Vec<AccessGroup>, PylosError> {
        let rows = self
            .pool
            .fetch_all("SELECT * FROM access_groups ORDER BY name", vec![])
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to list access groups: {}", e)))?;
        let mut list: Vec<AccessGroup> = rows.iter().map(row_to_access_group).collect();
        if let Some(t) = tag {
            list.retain(|ag| ag.tags.iter().any(|x| x == t));
        }
        Ok(list)
    }

    pub async fn get_access_group(&self, id: &str) -> Result<Option<AccessGroup>, PylosError> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT * FROM access_groups WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to get access group: {}", e)))?;
        Ok(row.as_ref().map(row_to_access_group))
    }

    pub async fn upsert_access_group(&self, ag: &AccessGroup) -> Result<(), PylosError> {
        let team_ids =
            serde_json::to_value(&ag.team_ids).unwrap_or(serde_json::Value::Array(vec![]));
        let user_ids =
            serde_json::to_value(&ag.user_ids).unwrap_or(serde_json::Value::Array(vec![]));
        let model_ids =
            serde_json::to_value(&ag.model_ids).unwrap_or(serde_json::Value::Array(vec![]));
        let provider_ids =
            serde_json::to_value(&ag.provider_ids).unwrap_or(serde_json::Value::Array(vec![]));
        let tags = serde_json::to_value(&ag.tags).unwrap_or(serde_json::Value::Array(vec![]));

        self.pool
            .execute(
                r#"INSERT INTO access_groups (id, name, description, organization_id, team_ids, user_ids, model_ids, provider_ids, is_active, tags, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    organization_id = excluded.organization_id,
                    team_ids = excluded.team_ids,
                    user_ids = excluded.user_ids,
                    model_ids = excluded.model_ids,
                    provider_ids = excluded.provider_ids,
                    is_active = excluded.is_active,
                    tags = excluded.tags,
                    updated_at = excluded.updated_at"#,
                vec![
                    DbValue::Text(ag.id.clone()),
                    DbValue::Text(ag.name.clone()),
                    DbValue::OptText(ag.description.clone()),
                    DbValue::OptText(ag.organization_id.clone()),
                    DbValue::Json(team_ids),
                    DbValue::Json(user_ids),
                    DbValue::Json(model_ids),
                    DbValue::Json(provider_ids),
                    DbValue::Bool(ag.is_active),
                    DbValue::Json(tags),
                    DbValue::I64(ag.created_at),
                    DbValue::I64(ag.updated_at),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to upsert access group: {}", e)))?;
        Ok(())
    }

    pub async fn delete_access_group(&self, id: &str) -> Result<bool, PylosError> {
        let rows = self
            .pool
            .execute(
                "DELETE FROM access_groups WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete access group: {}", e)))?;
        Ok(rows > 0)
    }

    // ── Policies ───────────────────────────────────────────────────────────────

    pub async fn list_policies(&self) -> Result<Vec<Policy>, PylosError> {
        let rows = self
            .pool
            .fetch_all("SELECT * FROM policies ORDER BY name", vec![])
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to list policies: {}", e)))?;
        Ok(rows.iter().map(row_to_policy).collect())
    }

    pub async fn upsert_policy(&self, policy: &Policy) -> Result<(), PylosError> {
        self.pool
            .execute(
                r#"INSERT INTO policies (id, name, description, policy_type, config, is_active, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    policy_type = excluded.policy_type,
                    config = excluded.config,
                    is_active = excluded.is_active,
                    updated_at = excluded.updated_at"#,
                vec![
                    DbValue::Text(policy.id.clone()),
                    DbValue::Text(policy.name.clone()),
                    DbValue::OptText(policy.description.clone()),
                    DbValue::Text(policy.policy_type.clone()),
                    DbValue::Json(policy.config.clone()),
                    DbValue::Bool(policy.is_active),
                    DbValue::I64(policy.created_at),
                    DbValue::I64(policy.updated_at),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to upsert policy: {}", e)))?;
        Ok(())
    }

    pub async fn delete_policy(&self, id: &str) -> Result<bool, PylosError> {
        let rows = self
            .pool
            .execute(
                "DELETE FROM policies WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete policy: {}", e)))?;
        Ok(rows > 0)
    }

    // ── Tool Policies ──────────────────────────────────────────────────────────

    pub async fn list_tool_policies(&self) -> Result<Vec<ToolPolicy>, PylosError> {
        let rows = self
            .pool
            .fetch_all("SELECT * FROM tool_policies ORDER BY name", vec![])
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to list tool policies: {}", e)))?;
        Ok(rows.iter().map(row_to_tool_policy).collect())
    }

    pub async fn upsert_tool_policy(&self, tp: &ToolPolicy) -> Result<(), PylosError> {
        let allowed_models =
            serde_json::to_value(&tp.allowed_models).unwrap_or(serde_json::Value::Array(vec![]));
        let allowed_providers = serde_json::to_value(&tp.allowed_providers)
            .unwrap_or(serde_json::Value::Array(vec![]));

        self.pool
            .execute(
                r#"INSERT INTO tool_policies (id, name, description, tool_type, allowed_models, allowed_providers, max_tokens_per_call, max_calls_per_minute, is_active, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    tool_type = excluded.tool_type,
                    allowed_models = excluded.allowed_models,
                    allowed_providers = excluded.allowed_providers,
                    max_tokens_per_call = excluded.max_tokens_per_call,
                    max_calls_per_minute = excluded.max_calls_per_minute,
                    is_active = excluded.is_active,
                    updated_at = excluded.updated_at"#,
                vec![
                    DbValue::Text(tp.id.clone()),
                    DbValue::Text(tp.name.clone()),
                    DbValue::OptText(tp.description.clone()),
                    DbValue::Text(tp.tool_type.clone()),
                    DbValue::Json(allowed_models),
                    DbValue::Json(allowed_providers),
                    DbValue::OptI64(tp.max_tokens_per_call),
                    DbValue::OptI64(tp.max_calls_per_minute),
                    DbValue::Bool(tp.is_active),
                    DbValue::I64(tp.created_at),
                    DbValue::I64(tp.updated_at),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to upsert tool policy: {}", e)))?;
        Ok(())
    }

    pub async fn delete_tool_policy(&self, id: &str) -> Result<bool, PylosError> {
        let rows = self
            .pool
            .execute(
                "DELETE FROM tool_policies WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete tool policy: {}", e)))?;
        Ok(rows > 0)
    }
}

#[async_trait]
impl OrganizationRepository for OrganizationStore {
    async fn list_organizations(&self) -> Result<Vec<Organization>, PylosError> {
        self.list_organizations(None).await
    }

    async fn get_organization(&self, id: &str) -> Result<Option<Organization>, PylosError> {
        self.get_organization(id).await
    }

    async fn upsert_organization(&self, org: &Organization) -> Result<(), PylosError> {
        self.upsert_organization(org).await
    }

    async fn delete_organization(&self, id: &str) -> Result<bool, PylosError> {
        self.delete_organization(id).await
    }
}

fn row_to_org(row: &DbRow) -> Organization {
    Organization {
        id: row.get_str("id"),
        name: row.get_str("name"),
        description: row.get_opt_str("description"),
        is_active: row.get_bool("is_active"),
        tags: row.get_json("tags").unwrap_or_default(),
        created_at: row.get_i64("created_at"),
        updated_at: row.get_i64("updated_at"),
    }
}

fn row_to_team(row: &DbRow) -> Team {
    Team {
        id: row.get_str("id"),
        organization_id: row.get_str("organization_id"),
        name: row.get_str("name"),
        description: row.get_opt_str("description"),
        is_active: row.get_bool("is_active"),
        tags: row.get_json("tags").unwrap_or_default(),
        created_at: row.get_i64("created_at"),
        updated_at: row.get_i64("updated_at"),
    }
}

fn row_to_user(row: &DbRow) -> InternalUser {
    InternalUser {
        id: row.get_str("id"),
        email: row.get_str("email"),
        name: row.get_str("name"),
        role: row.get_str("role"),
        group: row
            .get_opt_str("usr_group")
            .unwrap_or_else(|| "default".to_string()),
        organization_id: row.get_opt_str("organization_id"),
        team_ids: row.get_json("team_ids").unwrap_or_default(),
        is_active: row.get_bool("is_active"),
        created_at: row.get_i64("created_at"),
        updated_at: row.get_i64("updated_at"),
    }
}

fn row_to_access_group(row: &DbRow) -> AccessGroup {
    AccessGroup {
        id: row.get_str("id"),
        name: row.get_str("name"),
        description: row.get_opt_str("description"),
        organization_id: row.get_opt_str("organization_id"),
        team_ids: row.get_json("team_ids").unwrap_or_default(),
        user_ids: row.get_json("user_ids").unwrap_or_default(),
        model_ids: row.get_json("model_ids").unwrap_or_default(),
        provider_ids: row.get_json("provider_ids").unwrap_or_default(),
        is_active: row.get_bool("is_active"),
        tags: row.get_json("tags").unwrap_or_default(),
        created_at: row.get_i64("created_at"),
        updated_at: row.get_i64("updated_at"),
    }
}

fn row_to_policy(row: &DbRow) -> Policy {
    Policy {
        id: row.get_str("id"),
        name: row.get_str("name"),
        description: row.get_opt_str("description"),
        policy_type: row.get_str("policy_type"),
        config: row
            .get_json("config")
            .unwrap_or_else(|| serde_json::Value::Object(Default::default())),
        is_active: row.get_bool("is_active"),
        created_at: row.get_i64("created_at"),
        updated_at: row.get_i64("updated_at"),
    }
}

fn row_to_tool_policy(row: &DbRow) -> ToolPolicy {
    ToolPolicy {
        id: row.get_str("id"),
        name: row.get_str("name"),
        description: row.get_opt_str("description"),
        tool_type: row.get_str("tool_type"),
        allowed_models: row.get_json("allowed_models").unwrap_or_default(),
        allowed_providers: row.get_json("allowed_providers").unwrap_or_default(),
        max_tokens_per_call: row.get_opt_i64("max_tokens_per_call"),
        max_calls_per_minute: row.get_opt_i64("max_calls_per_minute"),
        is_active: row.get_bool("is_active"),
        created_at: row.get_i64("created_at"),
        updated_at: row.get_i64("updated_at"),
    }
}
