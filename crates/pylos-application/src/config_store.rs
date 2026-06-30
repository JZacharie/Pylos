use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use pylos_core::domain::config::{
    BedrockKeyConfig, EnvVar, NetworkConfig, ProviderConfig, ProviderKeyConfig, PylosConfig,
};
use pylos_core::domain::provider::ProviderConfig as RuntimeConfig;
use pylos_core::error::PylosError;

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;

// ─────────────────────────────────────────────────────────────────────────────
// Empreinte hash pour la réconciliation fichier ↔ mémoire
// Identique au mécanisme hash-based de bifrost
// ─────────────────────────────────────────────────────────────────────────────

fn hash_config(cfg: &PylosConfig) -> String {
    // On sérialise sans les compteurs d'usage pour qu'un changement de compteur
    // ne déclenche pas une resync (identique au comportement bifrost)
    let mut cfg_clone = cfg.clone();
    for budget in &mut cfg_clone.governance.budgets {
        budget.current_usage = 0.0;
    }
    match serde_json::to_string(&cfg_clone) {
        Ok(json) => {
            let mut hasher = Sha256::new();
            hasher.update(json.as_bytes());
            format!("{:x}", hasher.finalize())
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize config for hash — hot-reload reconciliation may be impaired");
            // Retourne une valeur fixe distincte de "" pour éviter de fausser la réconciliation
            "hash-error-fallback".to_string()
        }
    }
}

fn hash_provider(name: &str, provider: &ProviderConfig) -> String {
    let json = serde_json::to_string(provider).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update(json.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ─────────────────────────────────────────────────────────────────────────────
// ConfigStore — source de vérité en mémoire avec hot-reload
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum Pool {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

/// État interne du store
struct ConfigState {
    /// Config complète résolue
    config: PylosConfig,
    /// Hash de la config en mémoire (pour détecter les changements)
    config_hash: String,
    /// Hash par provider (pour le hot-reload partiel)
    provider_hashes: HashMap<String, String>,
    /// Providers runtime (avec clés résolues, prêts pour l'inférence)
    runtime_providers: Vec<(Arc<dyn pylos_core::domain::traits::Provider>, RuntimeConfig)>,
    /// Chemin du fichier source
    file_path: Option<PathBuf>,
    /// Pool de base de données (si configuré)
    db_pool: Option<Pool>,
}

/// Store de configuration partagé et rechargeable à chaud
/// Inspiré du ConfigStore de bifrost (framework/configstore/)
#[derive(Clone)]
pub struct ConfigStore {
    state: Arc<RwLock<ConfigState>>,
}

impl ConfigStore {
    /// Charge la config depuis un fichier pylos.json
    /// Si le fichier n'existe pas → auto-détection des providers via env vars
    pub async fn load(file_path: Option<&Path>) -> Result<Self, PylosError> {
        let (config, path) = match file_path {
            Some(p) if p.exists() => {
                info!(path = %p.display(), "Loading config from file");
                let raw = tokio::fs::read_to_string(p).await.map_err(|e| {
                    PylosError::Internal(format!("Failed to read config file: {}", e))
                })?;
                let cfg: PylosConfig = serde_json::from_str(&raw).map_err(|e| {
                    PylosError::Internal(format!("Invalid config file {}: {}", p.display(), e))
                })?;
                info!(
                    providers = cfg.providers.len(),
                    virtual_keys = cfg.governance.virtual_keys.len(),
                    plugins = cfg.plugins.len(),
                    "Config loaded"
                );
                (cfg, Some(p.to_path_buf()))
            }
            Some(p) => {
                warn!(path = %p.display(), "Config file not found, using auto-detection");
                (auto_detect_config(), None)
            }
            None => {
                info!("No config file specified, using auto-detection from environment");
                (auto_detect_config(), None)
            }
        };

        // Validation de base
        validate_config(&config)?;

        let config_hash = hash_config(&config);
        let mut provider_hashes = HashMap::new();
        for (name, provider) in &config.providers {
            provider_hashes.insert(name.clone(), hash_provider(name, provider));
        }

        // Construction des runtime providers depuis la config
        let runtime_providers = build_runtime_providers(&config);

        let state = ConfigState {
            config,
            config_hash,
            provider_hashes,
            runtime_providers,
            file_path: path,
            db_pool: None,
        };

        Ok(Self {
            state: Arc::new(RwLock::new(state)),
        })
    }

    /// Retourne le port configuré de façon async (remplace get_sync_port).
    /// blocking_read() est dangereux dans un contexte async Tokio (C-4 fix).
    pub async fn get_port(&self) -> u16 {
        self.state.read().await.config.server.port
    }

    /// Accès lecture à la config complète
    pub async fn get(&self) -> PylosConfig {
        self.state.read().await.config.clone()
    }

    /// Providers runtime prêts pour l'inférence
    pub async fn runtime_providers(
        &self,
    ) -> Vec<(Arc<dyn pylos_core::domain::traits::Provider>, RuntimeConfig)> {
        self.state.read().await.runtime_providers.clone()
    }

    /// Recharge la config depuis la base de données (si configurée) ou le fichier sur disque
    pub async fn reload(&self) -> Result<ReloadSummary, PylosError> {
        let db_pool = {
            let state = self.state.read().await;
            state.db_pool.clone()
        };

        if let Some(pool) = db_pool {
            let db_config: Option<String> = match &pool {
                Pool::Sqlite(p) => {
                    let row = sqlx::query("SELECT config FROM gateway_config WHERE id = 'pylos'")
                        .fetch_optional(p)
                        .await
                        .unwrap_or(None);
                    row.map(|r| r.get::<String, _>(0))
                }
                Pool::Postgres(p) => {
                    let row = sqlx::query("SELECT config FROM gateway_config WHERE id = 'pylos'")
                        .fetch_optional(p)
                        .await
                        .unwrap_or(None);
                    row.map(|r| {
                        let val: serde_json::Value = r.get(0);
                        val.to_string()
                    })
                }
            };

            if let Some(cfg_str) = db_config {
                let new_config: PylosConfig = serde_json::from_str(&cfg_str)
                    .map_err(|e| PylosError::Internal(format!("Invalid database config: {}", e)))?;

                validate_config(&new_config)?;
                let new_hash = hash_config(&new_config);

                let mut state = self.state.write().await;

                if new_hash == state.config_hash {
                    debug!("Config unchanged in database (hash match)");
                    return Ok(ReloadSummary {
                        changed: false,
                        providers_reloaded: vec![],
                        runtime_providers: state.runtime_providers.clone(),
                    });
                }

                let mut providers_reloaded = vec![];
                for (name, provider) in &new_config.providers {
                    let new_phash = hash_provider(name, provider);
                    let old_phash = state.provider_hashes.get(name).cloned().unwrap_or_default();
                    if new_phash != old_phash {
                        providers_reloaded.push(name.clone());
                        state.provider_hashes.insert(name.clone(), new_phash);
                    }
                }

                let runtime = build_runtime_providers(&new_config);
                info!(
                    providers_changed = providers_reloaded.len(),
                    "Config reloaded from database"
                );

                state.config = new_config;
                state.config_hash = new_hash;
                let runtime_for_return = runtime.clone();
                state.runtime_providers = runtime;

                return Ok(ReloadSummary {
                    changed: true,
                    providers_reloaded,
                    runtime_providers: runtime_for_return,
                });
            }
        }

        let file_path = {
            let state = self.state.read().await;
            state.file_path.clone()
        };

        let Some(path) = file_path else {
            return Err(PylosError::InvalidRequest(
                "No config file or database pool to reload".into(),
            ));
        };

        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to read config file: {}", e)))?;

        let new_config: PylosConfig = serde_json::from_str(&raw)
            .map_err(|e| PylosError::Internal(format!("Invalid config: {}", e)))?;

        validate_config(&new_config)?;

        let new_hash = hash_config(&new_config);

        let mut state = self.state.write().await;

        if new_hash == state.config_hash {
            debug!("Config unchanged (hash match), skipping reload");
            return Ok(ReloadSummary {
                changed: false,
                providers_reloaded: vec![],
                runtime_providers: state.runtime_providers.clone(),
            });
        }

        // Détecte quels providers ont changé
        let mut providers_reloaded = vec![];
        for (name, provider) in &new_config.providers {
            let new_phash = hash_provider(name, provider);
            let old_phash = state.provider_hashes.get(name).cloned().unwrap_or_default();
            if new_phash != old_phash {
                providers_reloaded.push(name.clone());
                state.provider_hashes.insert(name.clone(), new_phash);
            }
        }

        // Rebuild des runtime providers
        let runtime = build_runtime_providers(&new_config);

        info!(
            providers_changed = providers_reloaded.len(),
            "Config reloaded from file"
        );

        state.config = new_config;
        state.config_hash = new_hash;
        state.runtime_providers = runtime.clone();

        Ok(ReloadSummary {
            changed: true,
            providers_reloaded,
            runtime_providers: runtime,
        })
    }

    /// Initialise le pool de base de données et charge/synchronise la configuration.
    pub async fn init_database(&self, db_url: &str) -> Result<(), PylosError> {
        let pool = if db_url.starts_with("postgres") || db_url.starts_with("postgresql") {
            let p = PgPoolOptions::new()
                .max_connections(2)
                .connect(db_url)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to connect config Postgres: {}", e))
                })?;
            Pool::Postgres(p)
        } else {
            let p = SqlitePoolOptions::new()
                .max_connections(2)
                .connect(db_url)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to connect config SQLite: {}", e))
                })?;
            Pool::Sqlite(p)
        };

        match &pool {
            Pool::Sqlite(p) => {
                sqlx::query("CREATE TABLE IF NOT EXISTS gateway_config (id TEXT PRIMARY KEY, config TEXT NOT NULL)")
                    .execute(p)
                    .await
                    .ok();
            }
            Pool::Postgres(p) => {
                sqlx::query("CREATE TABLE IF NOT EXISTS gateway_config (id TEXT PRIMARY KEY, config JSONB NOT NULL)")
                    .execute(p)
                    .await
                    .ok();
            }
        }

        let db_config: Option<String> = match &pool {
            Pool::Sqlite(p) => {
                let row = sqlx::query("SELECT config FROM gateway_config WHERE id = 'pylos'")
                    .fetch_optional(p)
                    .await
                    .unwrap_or(None);
                row.map(|r| r.get::<String, _>(0))
            }
            Pool::Postgres(p) => {
                let row = sqlx::query("SELECT config FROM gateway_config WHERE id = 'pylos'")
                    .fetch_optional(p)
                    .await
                    .unwrap_or(None);
                row.map(|r| {
                    let val: serde_json::Value = r.get(0);
                    val.to_string()
                })
            }
        };

        let mut state = self.state.write().await;
        state.db_pool = Some(pool.clone());

        if let Some(cfg_str) = db_config {
            if let Ok(mut new_cfg) = serde_json::from_str::<PylosConfig>(&cfg_str) {
                info!("Loaded configuration from database; merging missing items from file config");
                let mut merged = false;

                // Merge missing providers
                for (name, provider) in &state.config.providers {
                    if !new_cfg.providers.contains_key(name) {
                        info!(
                            "Merging provider {} from file config into database config",
                            name
                        );
                        new_cfg.providers.insert(name.clone(), provider.clone());
                        merged = true;
                    }
                }

                // Merge missing virtual keys
                for vk in &state.config.governance.virtual_keys {
                    if !new_cfg
                        .governance
                        .virtual_keys
                        .iter()
                        .any(|v| v.id == vk.id)
                    {
                        info!(
                            "Merging virtual key {} from file config into database config",
                            vk.id
                        );
                        new_cfg.governance.virtual_keys.push(vk.clone());
                        merged = true;
                    }
                }

                // If merged, we write it back to the database so that it's persisted there
                if merged {
                    let json = serde_json::to_string(&new_cfg).unwrap_or_default();
                    match &pool {
                        Pool::Sqlite(p) => {
                            let _ = sqlx::query("INSERT INTO gateway_config (id, config) VALUES ('pylos', $1) ON CONFLICT(id) DO UPDATE SET config = excluded.config")
                                .bind(&json)
                                .execute(p)
                                .await;
                        }
                        Pool::Postgres(p) => {
                            let val: serde_json::Value =
                                serde_json::from_str(&json).unwrap_or_default();
                            let _ = sqlx::query("INSERT INTO gateway_config (id, config) VALUES ('pylos', $1) ON CONFLICT(id) DO UPDATE SET config = excluded.config")
                                .bind(&val)
                                .execute(p)
                                .await;
                        }
                    }
                }

                state.config = new_cfg;
                state.config_hash = hash_config(&state.config);
                state.provider_hashes.clear();
                let mut hashes = std::collections::HashMap::new();
                for (name, provider) in &state.config.providers {
                    hashes.insert(name.clone(), hash_provider(name, provider));
                }
                state.provider_hashes = hashes;
                state.runtime_providers = build_runtime_providers(&state.config);
            }
        } else {
            info!("No configuration found in database, inserting bootstrap configuration");
            let json = serde_json::to_string(&state.config).unwrap_or_default();
            match &pool {
                Pool::Sqlite(p) => {
                    let _ = sqlx::query("INSERT INTO gateway_config (id, config) VALUES ('pylos', $1) ON CONFLICT(id) DO UPDATE SET config = excluded.config")
                        .bind(&json)
                        .execute(p)
                        .await;
                }
                Pool::Postgres(p) => {
                    let val: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                    let _ = sqlx::query("INSERT INTO gateway_config (id, config) VALUES ('pylos', $1) ON CONFLICT(id) DO UPDATE SET config = excluded.config")
                        .bind(&val)
                        .execute(p)
                        .await;
                }
            }
        }

        Ok(())
    }

    /// Récupère le hash du mot de passe admin stocké en base de données.
    pub async fn get_admin_key_hash(&self) -> Result<Option<String>, PylosError> {
        let pool = {
            let state = self.state.read().await;
            state.db_pool.clone()
        };

        let pool = match pool {
            Some(p) => p,
            None => return Ok(None),
        };

        // Create table server_secrets if it does not exist
        match &pool {
            Pool::Sqlite(p) => {
                sqlx::query("CREATE TABLE IF NOT EXISTS server_secrets (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
                    .execute(p)
                    .await
                    .ok();
            }
            Pool::Postgres(p) => {
                sqlx::query("CREATE TABLE IF NOT EXISTS server_secrets (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
                    .execute(p)
                    .await
                    .ok();
            }
        }

        // Check if admin_key_hash exists
        let existing: Option<String> = match &pool {
            Pool::Sqlite(p) => {
                let row =
                    sqlx::query("SELECT value FROM server_secrets WHERE key = 'admin_key_hash'")
                        .fetch_optional(p)
                        .await
                        .unwrap_or(None);
                row.map(|r| r.get::<String, _>(0))
            }
            Pool::Postgres(p) => {
                let row =
                    sqlx::query("SELECT value FROM server_secrets WHERE key = 'admin_key_hash'")
                        .fetch_optional(p)
                        .await
                        .unwrap_or(None);
                row.map(|r| r.get::<String, _>(0))
            }
        };

        Ok(existing)
    }

    /// Enregistre le hash du mot de passe admin en base de données.
    pub async fn save_admin_key_hash(&self, hash: &str) -> Result<(), PylosError> {
        let pool = {
            let state = self.state.read().await;
            state.db_pool.clone()
        };

        let pool = match pool {
            Some(p) => p,
            None => {
                return Err(PylosError::Internal(
                    "No database connection pool configured".into(),
                ))
            }
        };

        // Create table server_secrets if it does not exist
        match &pool {
            Pool::Sqlite(p) => {
                sqlx::query("CREATE TABLE IF NOT EXISTS server_secrets (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
                    .execute(p)
                    .await
                    .ok();
            }
            Pool::Postgres(p) => {
                sqlx::query("CREATE TABLE IF NOT EXISTS server_secrets (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
                    .execute(p)
                    .await
                    .ok();
            }
        }

        // Insert or update hash
        match &pool {
            Pool::Sqlite(p) => {
                sqlx::query("INSERT INTO server_secrets (key, value) VALUES ('admin_key_hash', $1) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                    .bind(hash)
                    .execute(p)
                    .await
                    .map_err(|e| PylosError::Internal(format!("Failed to save admin key hash: {}", e)))?;
            }
            Pool::Postgres(p) => {
                sqlx::query("INSERT INTO server_secrets (key, value) VALUES ('admin_key_hash', $1) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                    .bind(hash)
                    .execute(p)
                    .await
                    .map_err(|e| PylosError::Internal(format!("Failed to save admin key hash: {}", e)))?;
            }
        }

        Ok(())
    }

    /// Met à jour un provider en mémoire et persiste sur disque
    pub async fn upsert_provider(
        &self,
        name: String,
        provider: ProviderConfig,
    ) -> Result<(), PylosError> {
        validate_provider(&name, &provider)?;
        let new_hash = hash_provider(&name, &provider);

        let mut state = self.state.write().await;
        state.config.providers.insert(name.clone(), provider);
        state.provider_hashes.insert(name, new_hash);
        state.runtime_providers = build_runtime_providers(&state.config);
        persist_config_locked(&state).await;

        Ok(())
    }

    /// Supprime un provider en mémoire et persiste sur disque
    /// Retourne true si le provider existait, false sinon
    pub async fn remove_provider(&self, name: &str) -> Result<bool, PylosError> {
        let mut state = self.state.write().await;
        let existed = state.config.providers.remove(name).is_some();
        if existed {
            state.provider_hashes.remove(name);
            state.runtime_providers = build_runtime_providers(&state.config);
            persist_config_locked(&state).await;
        }
        Ok(existed)
    }

    /// Ajoute une virtual key en mémoire et persiste sur disque
    pub async fn add_virtual_key(
        &self,
        vk: pylos_core::domain::config::VirtualKeyConfig,
    ) -> Result<(), PylosError> {
        let mut state = self.state.write().await;
        // Vérifie l'unicité de l'ID
        if state
            .config
            .governance
            .virtual_keys
            .iter()
            .any(|v| v.id == vk.id)
        {
            return Err(PylosError::InvalidRequest(format!(
                "Virtual key '{}' already exists",
                vk.id
            )));
        }
        state.config.governance.virtual_keys.push(vk);
        persist_config_locked(&state).await;
        Ok(())
    }

    /// Modifie une virtual key existante via une closure et persiste sur disque
    /// Retourne true si trouvée, false sinon
    pub async fn update_virtual_key(
        &self,
        id: &str,
        mutator: impl FnOnce(&mut pylos_core::domain::config::VirtualKeyConfig),
    ) -> Result<bool, PylosError> {
        let mut state = self.state.write().await;
        if let Some(vk) = state
            .config
            .governance
            .virtual_keys
            .iter_mut()
            .find(|v| v.id == id)
        {
            mutator(vk);
            persist_config_locked(&state).await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Supprime une virtual key en mémoire et persiste sur disque
    pub async fn remove_virtual_key(&self, id: &str) -> Result<bool, PylosError> {
        let mut state = self.state.write().await;
        let before = state.config.governance.virtual_keys.len();
        state.config.governance.virtual_keys.retain(|v| v.id != id);
        let removed = state.config.governance.virtual_keys.len() < before;
        if removed {
            persist_config_locked(&state).await;
        }
        Ok(removed)
    }

    /// Met à jour ou ajoute la configuration du plugin Guardrails et persiste
    pub async fn upsert_guardrails_config(
        &self,
        enabled: bool,
        config: serde_json::Value,
    ) -> Result<(), PylosError> {
        let mut state = self.state.write().await;

        let new_plugin = pylos_core::domain::config::PluginConfig {
            name: "guardrails".to_string(),
            enabled,
            config,
        };

        if let Some(pos) = state
            .config
            .plugins
            .iter()
            .position(|p| p.name == "guardrails")
        {
            state.config.plugins[pos] = new_plugin;
        } else {
            state.config.plugins.push(new_plugin);
        }

        persist_config_locked(&state).await;
        Ok(())
    }
}

/// Résultat d'un hot reload
pub struct ReloadSummary {
    pub changed: bool,
    pub providers_reloaded: Vec<String>,
    /// Les providers runtime après rechargement (évite un TOCTOU entre reload() et runtime_providers())
    pub runtime_providers: Vec<(Arc<dyn pylos_core::domain::traits::Provider>, RuntimeConfig)>,
}

impl std::fmt::Debug for ReloadSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReloadSummary")
            .field("changed", &self.changed)
            .field("providers_reloaded", &self.providers_reloaded)
            .field("runtime_providers_count", &self.runtime_providers.len())
            .finish()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Persistance sur disque
// ─────────────────────────────────────────────────────────────────────────────

/// Persiste la config actuelle sur disque.
/// Appelé avec le write-lock déjà tenu — donc prend une ref sur ConfigState.
/// En cas d'échec (pas de fichier configuré, erreur I/O), log un warning et continue.
async fn persist_config_locked(state: &ConfigState) {
    let json = match serde_json::to_string_pretty(&state.config) {
        Ok(j) => j,
        Err(e) => {
            warn!(error = %e, "Failed to serialize config for persistence");
            return;
        }
    };

    if let Some(pool) = &state.db_pool {
        match pool {
            Pool::Sqlite(p) => {
                let _ = sqlx::query("INSERT INTO gateway_config (id, config) VALUES ('pylos', $1) ON CONFLICT(id) DO UPDATE SET config = excluded.config")
                    .bind(&json)
                    .execute(p)
                    .await;
            }
            Pool::Postgres(p) => {
                let val: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                let _ = sqlx::query("INSERT INTO gateway_config (id, config) VALUES ('pylos', $1) ON CONFLICT(id) DO UPDATE SET config = excluded.config")
                    .bind(&val)
                    .execute(p)
                    .await;
            }
        }
        debug!("Config persisted to database");
    }

    let Some(path) = &state.file_path else {
        debug!("No config file path — skipping file persist");
        return;
    };

    if let Err(e) = tokio::fs::write(path, json.as_bytes()).await {
        warn!(path = %path.display(), error = %e, "Failed to persist config to disk");
    } else {
        debug!(path = %path.display(), "Config persisted to disk");
    }
}

fn add_provider_from_env(
    config: &mut PylosConfig,
    detected: &mut Vec<&'static str>,
    name: &'static str,
    env_key: &str,
    default_base_url: Option<&str>,
) {
    if let Ok(key) = std::env::var(env_key) {
        if !key.is_empty() {
            let mut provider = ProviderConfig {
                keys: vec![ProviderKeyConfig {
                    name: "default".into(),
                    value: EnvVar::Literal(key),
                    models: vec!["*".into()],
                    weight: 1.0,
                    bedrock_key_config: None,
                    azure_config: None,
                }],
                ..Default::default()
            };
            if let Some(url) = default_base_url {
                provider.network.base_url = Some(url.into());
            }
            config.providers.insert(name.into(), provider);
            detected.push(name);
        }
    }
}

fn auto_detect_config() -> PylosConfig {
    let mut config = PylosConfig::default();
    let mut detected = vec![];

    // OpenAI
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            let base_url = std::env::var("OPENAI_BASE_URL").ok();
            let provider = ProviderConfig {
                keys: vec![ProviderKeyConfig {
                    name: "default".into(),
                    value: EnvVar::Literal(key),
                    models: vec!["*".into()],
                    weight: 1.0,
                    bedrock_key_config: None,
                    azure_config: None,
                }],
                network: NetworkConfig {
                    base_url,
                    ..Default::default()
                },
                ..Default::default()
            };
            config.providers.insert("openai".into(), provider);
            detected.push("openai");
        }
    }

    add_provider_from_env(
        &mut config,
        &mut detected,
        "xai",
        "XAI_API_KEY",
        Some("https://api.x.ai/v1"),
    );
    add_provider_from_env(
        &mut config,
        &mut detected,
        "anthropic",
        "ANTHROPIC_API_KEY",
        None,
    );
    add_provider_from_env(
        &mut config,
        &mut detected,
        "mistral",
        "MISTRAL_API_KEY",
        Some("https://api.mistral.ai/v1"),
    );
    add_provider_from_env(
        &mut config,
        &mut detected,
        "groq",
        "GROQ_API_KEY",
        Some("https://api.groq.com/openai/v1"),
    );
    add_provider_from_env(
        &mut config,
        &mut detected,
        "deepseek",
        "DEEPSEEK_API_KEY",
        Some("https://api.deepseek.com"),
    );
    add_provider_from_env(
        &mut config,
        &mut detected,
        "perplexity",
        "PERPLEXITY_API_KEY",
        Some("https://api.perplexity.ai"),
    );
    add_provider_from_env(
        &mut config,
        &mut detected,
        "nebius",
        "NEBIUS_API_KEY",
        Some("https://api.studio.nebius.ai/v1"),
    );
    add_provider_from_env(
        &mut config,
        &mut detected,
        "cerebras",
        "CEREBRAS_API_KEY",
        Some("https://api.cerebras.ai/v1"),
    );

    // Ollama — auto-détection via OLLAMA_BASE_URL
    let ollama_url = std::env::var("OLLAMA_BASE_URL")
        .ok()
        .filter(|u| !u.is_empty());
    if ollama_url.is_some() || std::env::var("OLLAMA_API_KEY").is_ok() {
        let provider = ProviderConfig {
            keys: vec![ProviderKeyConfig {
                name: "default".into(),
                value: std::env::var("OLLAMA_API_KEY")
                    .ok()
                    .map(EnvVar::Literal)
                    .unwrap_or(EnvVar::Literal("ollama".into())),
                models: vec!["*".into()],
                weight: 1.0,
                bedrock_key_config: None,
                azure_config: None,
            }],
            network: NetworkConfig {
                base_url: ollama_url.or_else(|| Some("http://localhost:11434".into())),
                ..Default::default()
            },
            ..Default::default()
        };
        config.providers.insert("ollama".into(), provider);
        detected.push("ollama");
    }

    // Lemonade
    add_provider_from_env(
        &mut config,
        &mut detected,
        "lemonade",
        "LEMONADE_API_KEY",
        None,
    );
    // Priorité à AWS_ACCESS_KEY_ID + AWS_SECRET_ACCESS_KEY explicites
    // Sinon tente la chaîne de credentials par défaut (IAM role, IRSA, etc.)
    let has_aws_keys = std::env::var("AWS_ACCESS_KEY_ID").is_ok()
        || std::env::var("AWS_PROFILE").is_ok()
        || std::env::var("AWS_ROLE_ARN").is_ok()
        || std::path::Path::new(&format!(
            "{}/.aws/credentials",
            std::env::var("HOME").unwrap_or_default()
        ))
        .exists();

    if has_aws_keys {
        let region = std::env::var("AWS_REGION")
            .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "us-east-1".into());

        let bedrock_cfg = BedrockKeyConfig {
            access_key_id: std::env::var("AWS_ACCESS_KEY_ID")
                .ok()
                .filter(|k| !k.is_empty())
                .map(EnvVar::Literal),
            secret_access_key: std::env::var("AWS_SECRET_ACCESS_KEY")
                .ok()
                .filter(|k| !k.is_empty())
                .map(EnvVar::Literal),
            session_token: std::env::var("AWS_SESSION_TOKEN")
                .ok()
                .filter(|k| !k.is_empty())
                .map(EnvVar::Literal),
            region: region.clone(),
            role_arn: std::env::var("AWS_ROLE_ARN")
                .ok()
                .filter(|k| !k.is_empty())
                .map(EnvVar::Literal),
            ..BedrockKeyConfig::default()
        };

        config.providers.insert(
            "bedrock".into(),
            ProviderConfig {
                // Bedrock n'a pas de clé API au sens traditionnel — on met une entrée
                // fictive pour satisfaire la validation (les vraies creds sont dans bedrock_key_config)
                keys: vec![ProviderKeyConfig {
                    name: "default".into(),
                    value: EnvVar::Literal(String::new()),
                    models: vec!["*".into()],
                    weight: 1.0,
                    bedrock_key_config: Some(bedrock_cfg),
                    azure_config: None,
                }],
                ..Default::default()
            },
        );
        detected.push("bedrock");
    }

    if detected.is_empty() {
        warn!("No providers detected. Set OPENAI_API_KEY, ANTHROPIC_API_KEY, MISTRAL_API_KEY, GROQ_API_KEY, or AWS_ACCESS_KEY_ID");
    } else {
        info!(providers = ?detected, "Auto-detected providers from environment");
    }

    config
}

// ─────────────────────────────────────────────────────────────────────────────
// Construction des runtime providers depuis la config
// ─────────────────────────────────────────────────────────────────────────────

fn build_runtime_providers(
    config: &PylosConfig,
) -> Vec<(Arc<dyn pylos_core::domain::traits::Provider>, RuntimeConfig)> {
    use pylos_core::domain::provider::{ProviderConfig as RuntimeCfg, ProviderKey, ProviderKind};
    use pylos_infrastructure::{
        AnthropicProvider, AzureProvider, BedrockProvider, CohereProvider, GeminiProvider,
        OpenAIProvider,
    };

    let mut providers = vec![];

    for (name, provider_cfg) in &config.providers {
        let mut allowed_models = Vec::new();
        for key_cfg in &provider_cfg.keys {
            for m in &key_cfg.models {
                if !allowed_models.contains(m) {
                    allowed_models.push(m.clone());
                }
            }
        }

        // ── Cas Bedrock : gestion séparée (pas de clé API traditionnelle) ──
        if name == "bedrock" {
            // Extrait la BedrockKeyConfig depuis la première clé
            let raw_bedrock_cfg = provider_cfg
                .keys
                .first()
                .and_then(|k| k.bedrock_key_config.clone());

            // Résolution des EnvVar dans la config Bedrock
            // La région peut être "env.AWS_REGION" → doit être résolue maintenant
            let mut bedrock_cfg = raw_bedrock_cfg.unwrap_or_default();

            // Résolution de la région si elle contient une référence env
            if bedrock_cfg.region.starts_with("env.") {
                let var_name = bedrock_cfg.region.trim_start_matches("env.");
                bedrock_cfg.region = std::env::var(var_name).unwrap_or_else(|_| "us-east-1".into());
            }

            // Résolution des autres champs EnvVar
            let resolved_access_key = bedrock_cfg.access_key_id.as_ref().and_then(|e| e.resolve());
            let resolved_secret_key = bedrock_cfg
                .secret_access_key
                .as_ref()
                .and_then(|e| e.resolve());

            if let Some(ak) = resolved_access_key {
                bedrock_cfg.access_key_id = Some(pylos_core::domain::config::EnvVar::Literal(ak));
            }
            if let Some(sk) = resolved_secret_key {
                bedrock_cfg.secret_access_key =
                    Some(pylos_core::domain::config::EnvVar::Literal(sk));
            }

            // role_arn : si la var d'env est absente → mettre à None pour éviter l'AssumeRole
            if let Some(ref arn_env) = bedrock_cfg.role_arn.clone() {
                match arn_env.resolve() {
                    Some(arn) if !arn.is_empty() => {
                        bedrock_cfg.role_arn =
                            Some(pylos_core::domain::config::EnvVar::Literal(arn));
                    }
                    _ => {
                        bedrock_cfg.role_arn = None; // pas d'AssumeRole
                    }
                }
            }

            let region = bedrock_cfg.region.clone();
            let keys = vec![ProviderKey::new("bedrock-iam").with_weight(1.0)];
            let mut runtime_cfg = RuntimeCfg::new(ProviderKind::Bedrock, keys);
            runtime_cfg.timeout_ms = provider_cfg.network.timeout_secs * 1000;
            runtime_cfg.max_retries = provider_cfg.network.max_retries;
            runtime_cfg.retry_backoff_initial_ms = provider_cfg.network.retry_backoff_initial_ms;
            runtime_cfg.retry_backoff_max_ms = provider_cfg.network.retry_backoff_max_ms;
            runtime_cfg.bedrock = Some(bedrock_cfg);
            runtime_cfg.allowed_models = allowed_models.clone();

            info!(provider = "bedrock", region = %region, "Bedrock provider registered");
            providers.push((
                Arc::new(BedrockProvider::new()) as Arc<dyn pylos_core::domain::traits::Provider>,
                runtime_cfg,
            ));
            continue;
        }

        // ── Cas Azure OpenAI ──
        if name == "azure" || provider_cfg.keys.iter().any(|k| k.azure_config.is_some()) {
            let keys: Vec<ProviderKey> = provider_cfg
                .keys
                .iter()
                .filter_map(|k| {
                    k.value
                        .resolve()
                        .map(|v| ProviderKey::new(v).with_weight(k.weight))
                })
                .collect();

            if keys.is_empty() {
                warn!(provider = %name, "No resolvable Azure API keys, skipping provider");
                continue;
            }

            // Extrait la config Azure depuis la première clé qui en a une
            // AzureKeyConfig est maintenant un alias de AzureConfig — pas de conversion nécessaire
            let azure_cfg = provider_cfg
                .keys
                .iter()
                .find_map(|k| k.azure_config.clone());

            if azure_cfg.is_none() {
                warn!(provider = %name, "Azure provider requires azure_config in at least one key, skipping");
                continue;
            }

            let mut runtime_cfg = RuntimeCfg::new(ProviderKind::Azure, keys);
            runtime_cfg.timeout_ms = provider_cfg.network.timeout_secs * 1000;
            runtime_cfg.max_retries = provider_cfg.network.max_retries;
            runtime_cfg.retry_backoff_initial_ms = provider_cfg.network.retry_backoff_initial_ms;
            runtime_cfg.retry_backoff_max_ms = provider_cfg.network.retry_backoff_max_ms;
            runtime_cfg.azure = azure_cfg;
            runtime_cfg.allowed_models = allowed_models.clone();

            info!(provider = %name, "Azure OpenAI provider registered");
            providers.push((
                Arc::new(AzureProvider::new()) as Arc<dyn pylos_core::domain::traits::Provider>,
                runtime_cfg,
            ));
            continue;
        }

        // ── Providers HTTP classiques (OpenAI, Anthropic, Groq, etc.) ──
        let keys: Vec<ProviderKey> = provider_cfg
            .keys
            .iter()
            .filter_map(|k| {
                k.value
                    .resolve()
                    .map(|v| ProviderKey::new(v).with_weight(k.weight))
            })
            .collect();

        if keys.is_empty() {
            warn!(provider = %name, "No resolvable keys, skipping provider");
            continue;
        }

        let kind = match name.as_str() {
            "openai" => ProviderKind::OpenAI,
            "anthropic" => ProviderKind::Anthropic,
            "gemini" | "google" => ProviderKind::Gemini,
            "cohere" => ProviderKind::Cohere,
            "groq" => ProviderKind::Groq,
            "mistral" => ProviderKind::Mistral,
            "cerebras" => ProviderKind::Cerebras,
            "perplexity" => ProviderKind::Perplexity,
            "fireworks" => ProviderKind::Fireworks,
            "xai" | "x-ai" => ProviderKind::XAI,
            "nebius" => ProviderKind::Nebius,
            "deepseek" => ProviderKind::DeepSeek,
            "ollama-jo3" | "ollama" => ProviderKind::Ollama,
            "openrouter" => ProviderKind::OpenRouter,
            "lemonade" => ProviderKind::Lemonade,
            other => ProviderKind::Custom(other.to_string()),
        };

        // Applique l'URL de base par défaut si aucune n'est configurée
        let base_url = provider_cfg.network.base_url.clone().or_else(|| {
            pylos_core::domain::provider::default_base_url(&kind).map(|s| s.to_string())
        });

        let mut runtime_cfg = RuntimeCfg::new(kind.clone(), keys);
        runtime_cfg.base_url = base_url;
        runtime_cfg.timeout_ms = provider_cfg.network.timeout_secs * 1000;
        runtime_cfg.max_retries = provider_cfg.network.max_retries;
        runtime_cfg.retry_backoff_initial_ms = provider_cfg.network.retry_backoff_initial_ms;
        runtime_cfg.retry_backoff_max_ms = provider_cfg.network.retry_backoff_max_ms;
        runtime_cfg.bedrock = None;
        runtime_cfg.azure = None;
        runtime_cfg.allowed_models = allowed_models.clone();

        let provider: Arc<dyn pylos_core::domain::traits::Provider> = match kind {
            ProviderKind::Anthropic => Arc::new(AnthropicProvider::new()),
            ProviderKind::Gemini => Arc::new(GeminiProvider::new()),
            ProviderKind::Cohere => Arc::new(CohereProvider::new()),
            _ => Arc::new(OpenAIProvider::new(name.to_string())), // OpenAI-compatibles : OpenAI, Groq, Mistral, xAI, etc.
        };

        info!(provider = %name, keys = provider_cfg.keys.len(), "Provider registered");
        providers.push((provider, runtime_cfg));
    }

    providers
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation
// ─────────────────────────────────────────────────────────────────────────────

fn validate_config(config: &PylosConfig) -> Result<(), PylosError> {
    // Version supportée
    if config.version > 2 {
        return Err(PylosError::InvalidRequest(format!(
            "Unsupported config version: {}. Supported: 1, 2",
            config.version
        )));
    }

    // Validation de chaque provider
    for (name, provider) in &config.providers {
        validate_provider(name, provider)?;
    }

    // Validation des virtual keys
    let vk_ids: std::collections::HashSet<_> = config
        .governance
        .virtual_keys
        .iter()
        .map(|v| &v.id)
        .collect();
    if vk_ids.len() != config.governance.virtual_keys.len() {
        return Err(PylosError::InvalidRequest(
            "Duplicate virtual key IDs in config".into(),
        ));
    }

    Ok(())
}

fn validate_provider(name: &str, provider: &ProviderConfig) -> Result<(), PylosError> {
    if name.is_empty() {
        return Err(PylosError::InvalidRequest(
            "Provider name cannot be empty".into(),
        ));
    }
    for key in &provider.keys {
        if key.name.is_empty() {
            return Err(PylosError::InvalidRequest(format!(
                "Key name cannot be empty in provider '{}'",
                name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_config_file(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[tokio::test]
    async fn test_load_from_file() {
        let f = make_config_file(
            r#"{
            "providers": {
                "openai": {
                    "keys": [{"name": "k1", "value": "sk-test"}]
                }
            }
        }"#,
        );
        let store = ConfigStore::load(Some(f.path())).await.unwrap();
        let cfg = store.get().await;
        assert!(cfg.providers.contains_key("openai"));
    }

    #[tokio::test]
    async fn test_load_no_file_auto_detect() {
        // Sans fichier et sans env vars — doit fonctionner sans erreur
        let store = ConfigStore::load(None).await.unwrap();
        let cfg = store.get().await;
        // Pas de provider configuré (pas de clés dans l'env de test)
        let _ = cfg;
    }

    #[tokio::test]
    async fn test_hot_reload_unchanged() {
        let f = make_config_file(
            r#"{
            "providers": {
                "openai": {"keys": [{"name": "k1", "value": "sk-test"}]}
            }
        }"#,
        );
        let store = ConfigStore::load(Some(f.path())).await.unwrap();
        let summary = store.reload().await.unwrap();
        assert!(!summary.changed, "Hash unchanged, reload should be no-op");
    }

    #[tokio::test]
    async fn test_validation_rejects_duplicate_vk_ids() {
        let f = make_config_file(
            r#"{
            "governance": {
                "virtual_keys": [
                    {"id": "vk-1", "name": "A"},
                    {"id": "vk-1", "name": "B"}
                ]
            }
        }"#,
        );
        let result = ConfigStore::load(Some(f.path())).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_ignores_budget_usage() {
        let mut cfg = PylosConfig::default();
        cfg.governance
            .budgets
            .push(pylos_core::domain::config::BudgetConfig {
                id: "b1".into(),
                max_limit: 100.0,
                reset_duration: pylos_core::domain::config::Duration("1d".into()),
                current_usage: 0.0,
                virtual_key_id: None,
            });
        let h1 = hash_config(&cfg);

        cfg.governance.budgets[0].current_usage = 42.5;
        let h2 = hash_config(&cfg);

        assert_eq!(
            h1, h2,
            "Hash should be identical regardless of current_usage"
        );
    }
}
