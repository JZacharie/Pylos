use std::path::Path;

use serde::de::DeserializeOwned;
use sqlx::postgres::{PgArguments, PgPool, PgPoolOptions, PgRow};
use sqlx::sqlite::{SqliteArguments, SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::{Arguments, Row};
use tracing::info;

/// Valeur typée portable pour binder des paramètres SQL sur SQLite et PostgreSQL
/// sans dupliquer les requêtes par dialecte.
#[derive(Debug, Clone)]
pub enum DbValue {
    Text(String),
    OptText(Option<String>),
    I64(i64),
    OptI64(Option<i64>),
    F64(f64),
    OptF64(Option<f64>),
    Bool(bool),
    Json(serde_json::Value),
    OptJson(Option<serde_json::Value>),
}

impl From<&str> for DbValue {
    fn from(v: &str) -> Self {
        Self::Text(v.to_string())
    }
}

impl From<String> for DbValue {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<&String> for DbValue {
    fn from(v: &String) -> Self {
        Self::Text(v.clone())
    }
}

impl From<Option<String>> for DbValue {
    fn from(v: Option<String>) -> Self {
        Self::OptText(v)
    }
}

impl From<i64> for DbValue {
    fn from(v: i64) -> Self {
        Self::I64(v)
    }
}

impl From<Option<i64>> for DbValue {
    fn from(v: Option<i64>) -> Self {
        Self::OptI64(v)
    }
}

impl From<f64> for DbValue {
    fn from(v: f64) -> Self {
        Self::F64(v)
    }
}

impl From<bool> for DbValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

/// Ligne SQL unifiée (SQLite ou PostgreSQL) avec accesseurs portables.
pub enum DbRow {
    Sqlite(SqliteRow),
    Postgres(PgRow),
}

impl DbRow {
    pub fn get_str(&self, col: &str) -> String {
        match self {
            Self::Sqlite(r) => r.try_get(col).unwrap_or_default(),
            Self::Postgres(r) => r.try_get(col).unwrap_or_default(),
        }
    }

    pub fn get_opt_str(&self, col: &str) -> Option<String> {
        match self {
            Self::Sqlite(r) => r.try_get(col).ok().flatten(),
            Self::Postgres(r) => r.try_get(col).ok().flatten(),
        }
    }

    pub fn get_i64(&self, col: &str) -> i64 {
        match self {
            Self::Sqlite(r) => r.try_get(col).unwrap_or(0),
            Self::Postgres(r) => r
                .try_get::<i64, _>(col)
                .or_else(|_| r.try_get::<i32, _>(col).map(i64::from))
                .unwrap_or(0),
        }
    }

    pub fn get_opt_i64(&self, col: &str) -> Option<i64> {
        match self {
            Self::Sqlite(r) => r.try_get(col).ok().flatten(),
            Self::Postgres(r) => r
                .try_get::<Option<i64>, _>(col)
                .ok()
                .flatten()
                .or_else(|| r.try_get::<Option<i32>, _>(col).ok().flatten().map(i64::from)),
        }
    }

    pub fn get_f64(&self, col: &str) -> f64 {
        match self {
            Self::Sqlite(r) => r.try_get(col).unwrap_or(0.0),
            Self::Postgres(r) => r.try_get(col).unwrap_or(0.0),
        }
    }

    pub fn get_opt_f64(&self, col: &str) -> Option<f64> {
        match self {
            Self::Sqlite(r) => r.try_get(col).ok().flatten(),
            Self::Postgres(r) => r.try_get(col).ok().flatten(),
        }
    }

    pub fn get_bool(&self, col: &str) -> bool {
        match self {
            Self::Sqlite(r) => r
                .try_get::<bool, _>(col)
                .or_else(|_| r.try_get::<i64, _>(col).map(|v| v != 0))
                .unwrap_or(false),
            Self::Postgres(r) => r
                .try_get::<bool, _>(col)
                .or_else(|_| r.try_get::<i64, _>(col).map(|v| v != 0))
                .unwrap_or(false),
        }
    }

    pub fn get_json<T: DeserializeOwned>(&self, col: &str) -> Option<T> {
        match self {
            Self::Sqlite(r) => r
                .try_get::<String, _>(col)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok()),
            Self::Postgres(r) => r
                .try_get::<serde_json::Value, _>(col)
                .ok()
                .and_then(|v| serde_json::from_value(v).ok())
                .or_else(|| {
                    r.try_get::<String, _>(col)
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                }),
        }
    }
}

fn bind_sqlite<'q>(params: &'q [DbValue]) -> SqliteArguments<'q> {
    let mut args = SqliteArguments::default();
    for p in params {
        match p {
            DbValue::Text(v) => {
                let _ = args.add(v.as_str());
            }
            DbValue::OptText(v) => {
                let _ = args.add(v.as_deref());
            }
            DbValue::I64(v) => {
                let _ = args.add(*v);
            }
            DbValue::OptI64(v) => {
                let _ = args.add(*v);
            }
            DbValue::F64(v) => {
                let _ = args.add(*v);
            }
            DbValue::OptF64(v) => {
                let _ = args.add(*v);
            }
            DbValue::Bool(v) => {
                let _ = args.add(*v);
            }
            DbValue::Json(v) => {
                let _ = args.add(v.to_string());
            }
            DbValue::OptJson(v) => {
                let _ = args.add(v.as_ref().map(|x| x.to_string()));
            }
        }
    }
    args
}

fn bind_pg(params: &[DbValue]) -> PgArguments {
    let mut args = PgArguments::default();
    for p in params {
        match p {
            DbValue::Text(v) => {
                let _ = args.add(v.as_str());
            }
            DbValue::OptText(v) => {
                let _ = args.add(v.as_deref());
            }
            DbValue::I64(v) => {
                let _ = args.add(*v);
            }
            DbValue::OptI64(v) => {
                let _ = args.add(*v);
            }
            DbValue::F64(v) => {
                let _ = args.add(*v);
            }
            DbValue::OptF64(v) => {
                let _ = args.add(*v);
            }
            DbValue::Bool(v) => {
                let _ = args.add(*v);
            }
            DbValue::Json(v) => {
                let _ = args.add(v);
            }
            DbValue::OptJson(v) => {
                let _ = args.add(v.as_ref());
            }
        }
    }
    args
}

/// Pool de base de données unifié pour tous les stores (SQLite ou PostgreSQL).
/// Centralise le dispatch SQLite/Postgres pour éliminer la duplication dans les stores.
#[derive(Clone)]
pub enum DbPool {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

impl DbPool {
    pub async fn open_sqlite(
        db_path: &Path,
        pool_name: &str,
        max_connections: u32,
    ) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect_with(options)
            .await?;

        info!(path = %db_path.display(), pool = %pool_name, "Store opened (SQLite)");
        Ok(DbPool::Sqlite(pool))
    }

    pub async fn open_postgres(database_url: &str, pool_name: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(database_url)
            .await?;

        info!(pool = %pool_name, "Store opened (PostgreSQL)");
        Ok(DbPool::Postgres(pool))
    }

    pub async fn in_memory(max_connections: u32) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect("sqlite::memory:")
            .await?;
        Ok(DbPool::Sqlite(pool))
    }

    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        match self {
            DbPool::Sqlite(pool) => sqlx::migrate!("./migrations")
                .run(pool)
                .await
                .map_err(|e| sqlx::Error::Migrate(Box::new(e))),
            DbPool::Postgres(pool) => sqlx::migrate!("./migrations_postgres")
                .run(pool)
                .await
                .map_err(|e| sqlx::Error::Migrate(Box::new(e))),
        }
    }

    pub async fn execute(&self, sql: &str, params: Vec<DbValue>) -> Result<u64, sqlx::Error> {
        match self {
            DbPool::Sqlite(pool) => {
                let args = bind_sqlite(&params);
                let res = sqlx::query_with(sql, args).execute(pool).await?;
                Ok(res.rows_affected())
            }
            DbPool::Postgres(pool) => {
                let args = bind_pg(&params);
                let res = sqlx::query_with(sql, args).execute(pool).await?;
                Ok(res.rows_affected())
            }
        }
    }

    pub async fn fetch_optional(
        &self,
        sql: &str,
        params: Vec<DbValue>,
    ) -> Result<Option<DbRow>, sqlx::Error> {
        match self {
            DbPool::Sqlite(pool) => {
                let args = bind_sqlite(&params);
                let row = sqlx::query_with(sql, args).fetch_optional(pool).await?;
                Ok(row.map(DbRow::Sqlite))
            }
            DbPool::Postgres(pool) => {
                let args = bind_pg(&params);
                let row = sqlx::query_with(sql, args).fetch_optional(pool).await?;
                Ok(row.map(DbRow::Postgres))
            }
        }
    }

    pub async fn fetch_all(
        &self,
        sql: &str,
        params: Vec<DbValue>,
    ) -> Result<Vec<DbRow>, sqlx::Error> {
        match self {
            DbPool::Sqlite(pool) => {
                let args = bind_sqlite(&params);
                let rows = sqlx::query_with(sql, args).fetch_all(pool).await?;
                Ok(rows.into_iter().map(DbRow::Sqlite).collect())
            }
            DbPool::Postgres(pool) => {
                let args = bind_pg(&params);
                let rows = sqlx::query_with(sql, args).fetch_all(pool).await?;
                Ok(rows.into_iter().map(DbRow::Postgres).collect())
            }
        }
    }

    pub fn as_sqlite(&self) -> Option<&SqlitePool> {
        match self {
            DbPool::Sqlite(p) => Some(p),
            DbPool::Postgres(_) => None,
        }
    }

    pub async fn begin(&self) -> Result<DbTx<'_>, sqlx::Error> {
        match self {
            DbPool::Sqlite(pool) => Ok(DbTx::Sqlite(pool.begin().await?)),
            DbPool::Postgres(pool) => Ok(DbTx::Postgres(pool.begin().await?)),
        }
    }
}

/// Transaction SQL unifiée (SQLite ou PostgreSQL).
pub enum DbTx<'a> {
    Sqlite(sqlx::Transaction<'a, sqlx::Sqlite>),
    Postgres(sqlx::Transaction<'a, sqlx::Postgres>),
}

impl<'a> DbTx<'a> {
    pub async fn fetch_optional(
        &mut self,
        sql: &str,
        params: Vec<DbValue>,
    ) -> Result<Option<DbRow>, sqlx::Error> {
        match self {
            Self::Sqlite(tx) => {
                let args = bind_sqlite(&params);
                let row = sqlx::query_with(sql, args).fetch_optional(&mut **tx).await?;
                Ok(row.map(DbRow::Sqlite))
            }
            Self::Postgres(tx) => {
                let args = bind_pg(&params);
                let row = sqlx::query_with(sql, args).fetch_optional(&mut **tx).await?;
                Ok(row.map(DbRow::Postgres))
            }
        }
    }

    pub async fn execute(&mut self, sql: &str, params: Vec<DbValue>) -> Result<u64, sqlx::Error> {
        match self {
            Self::Sqlite(tx) => {
                let args = bind_sqlite(&params);
                let res = sqlx::query_with(sql, args).execute(&mut **tx).await?;
                Ok(res.rows_affected())
            }
            Self::Postgres(tx) => {
                let args = bind_pg(&params);
                let res = sqlx::query_with(sql, args).execute(&mut **tx).await?;
                Ok(res.rows_affected())
            }
        }
    }

    pub async fn commit(self) -> Result<(), sqlx::Error> {
        match self {
            Self::Sqlite(tx) => tx.commit().await,
            Self::Postgres(tx) => tx.commit().await,
        }
    }

    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        match self {
            Self::Sqlite(tx) => tx.rollback().await,
            Self::Postgres(tx) => tx.rollback().await,
        }
    }
}
