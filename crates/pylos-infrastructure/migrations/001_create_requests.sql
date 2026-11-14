-- Migration 001: table de logs des requêtes d'inférence
-- Remplace le ring buffer in-memory du LogStore
-- Compatible avec le schéma logstore de bifrost (framework/logstore/)

CREATE TABLE IF NOT EXISTS requests (
    id              TEXT    PRIMARY KEY,
    timestamp       INTEGER NOT NULL,       -- Unix milliseconds
    provider        TEXT    NOT NULL,
    model           TEXT    NOT NULL,
    object          TEXT    NOT NULL,       -- "chat.completion" | "chat.completion.stream"
    status          TEXT    NOT NULL,       -- "success" | "error"
    latency_ms      REAL    NOT NULL,
    prompt_tokens   INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens    INTEGER NOT NULL DEFAULT 0,
    cost_usd        REAL    NOT NULL DEFAULT 0.0,
    finish_reason   TEXT,
    error_message   TEXT,
    virtual_key     TEXT,
    is_stream       INTEGER NOT NULL DEFAULT 0,  -- 0 = false, 1 = true
    input_preview   TEXT,
    output_preview  TEXT
);

CREATE INDEX IF NOT EXISTS idx_requests_timestamp   ON requests(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_requests_provider    ON requests(provider);
CREATE INDEX IF NOT EXISTS idx_requests_model       ON requests(model);
CREATE INDEX IF NOT EXISTS idx_requests_status      ON requests(status);
CREATE INDEX IF NOT EXISTS idx_requests_virtual_key ON requests(virtual_key);
