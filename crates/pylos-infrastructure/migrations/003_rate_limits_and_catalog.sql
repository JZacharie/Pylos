-- Migration 003: rate limits persistants par virtual key
-- Remplace le sliding-window in-memory de VirtualKeyRegistry
-- Bifrost source: plugins/governance/ratelimit.go

CREATE TABLE IF NOT EXISTS vk_rate_limits (
    virtual_key_id      TEXT    NOT NULL,
    window_type         TEXT    NOT NULL,  -- "rpm" | "tpm" | "rpd" | "tpd"
    max_value           INTEGER NOT NULL,  -- 0 = illimité
    current_value       INTEGER NOT NULL DEFAULT 0,
    window_start_ms     INTEGER NOT NULL,  -- début de la fenêtre courante
    window_duration_ms  INTEGER NOT NULL,  -- durée en ms
    PRIMARY KEY (virtual_key_id, window_type)
);

CREATE INDEX IF NOT EXISTS idx_rl_vk ON vk_rate_limits(virtual_key_id);

-- Migration 004: model catalog
CREATE TABLE IF NOT EXISTS model_catalog (
    id                      TEXT    PRIMARY KEY,   -- "{provider}/{model_id}"
    provider                TEXT    NOT NULL,
    model_id                TEXT    NOT NULL,
    display_name            TEXT,
    context_window          INTEGER NOT NULL DEFAULT 0,
    max_output_tokens       INTEGER NOT NULL DEFAULT 0,
    input_price_per_1m_usd  REAL    NOT NULL DEFAULT 0.0,
    output_price_per_1m_usd REAL    NOT NULL DEFAULT 0.0,
    supports_vision         INTEGER NOT NULL DEFAULT 0,
    supports_tools          INTEGER NOT NULL DEFAULT 1,
    supports_streaming      INTEGER NOT NULL DEFAULT 1,
    supports_embeddings     INTEGER NOT NULL DEFAULT 0,
    is_deprecated           INTEGER NOT NULL DEFAULT 0,
    updated_at_ms           INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_catalog_provider ON model_catalog(provider);
CREATE UNIQUE INDEX IF NOT EXISTS idx_catalog_uniq ON model_catalog(provider, model_id);
