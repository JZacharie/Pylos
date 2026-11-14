CREATE TABLE IF NOT EXISTS vk_rate_limits (
    virtual_key_id      TEXT    NOT NULL,
    window_type         TEXT    NOT NULL,
    max_value           INTEGER NOT NULL,
    current_value       INTEGER NOT NULL DEFAULT 0,
    window_start_ms     BIGINT  NOT NULL,
    window_duration_ms  BIGINT  NOT NULL,
    PRIMARY KEY (virtual_key_id, window_type)
);

CREATE INDEX IF NOT EXISTS idx_rl_vk ON vk_rate_limits(virtual_key_id);

CREATE TABLE IF NOT EXISTS model_catalog (
    id                      TEXT    PRIMARY KEY,
    provider                TEXT    NOT NULL,
    model_id                TEXT    NOT NULL,
    display_name            TEXT,
    context_window          INTEGER NOT NULL DEFAULT 0,
    max_output_tokens       INTEGER NOT NULL DEFAULT 0,
    input_price_per_1m_usd  DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    output_price_per_1m_usd DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    supports_vision         SMALLINT NOT NULL DEFAULT 0,
    supports_tools          SMALLINT NOT NULL DEFAULT 1,
    supports_streaming      SMALLINT NOT NULL DEFAULT 1,
    supports_embeddings     SMALLINT NOT NULL DEFAULT 0,
    is_deprecated           SMALLINT NOT NULL DEFAULT 0,
    updated_at_ms           BIGINT  NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_catalog_provider ON model_catalog(provider);
CREATE UNIQUE INDEX IF NOT EXISTS idx_catalog_uniq ON model_catalog(provider, model_id);
