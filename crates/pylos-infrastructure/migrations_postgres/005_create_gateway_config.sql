CREATE TABLE IF NOT EXISTS gateway_config (
    id     TEXT PRIMARY KEY,
    config JSONB NOT NULL
);
