CREATE TABLE IF NOT EXISTS virtual_keys (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    description       TEXT,
    value             TEXT NOT NULL UNIQUE,
    is_active         INTEGER NOT NULL DEFAULT 1,
    rate_limit_id     TEXT,
    provider_configs  TEXT -- Stocké au format JSON text
);
