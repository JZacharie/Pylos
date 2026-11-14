CREATE TABLE IF NOT EXISTS virtual_keys (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    description       TEXT,
    value             TEXT NOT NULL UNIQUE,
    is_active         BOOLEAN NOT NULL DEFAULT TRUE,
    rate_limit_id     TEXT,
    provider_configs  JSONB -- Support natif JSONB de Postgres
);
