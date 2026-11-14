CREATE TABLE IF NOT EXISTS requests (
    id                TEXT    PRIMARY KEY,
    timestamp         BIGINT  NOT NULL,
    provider          TEXT    NOT NULL DEFAULT '',
    model             TEXT    NOT NULL DEFAULT '',
    object            TEXT    NOT NULL DEFAULT '',
    status            TEXT    NOT NULL DEFAULT 'success',
    latency_ms        DOUBLE PRECISION NOT NULL DEFAULT 0,
    prompt_tokens     INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens      INTEGER NOT NULL DEFAULT 0,
    cost_usd          DOUBLE PRECISION NOT NULL DEFAULT 0,
    finish_reason     TEXT,
    error_message     TEXT,
    virtual_key       TEXT,
    is_stream         SMALLINT NOT NULL DEFAULT 0,
    input_preview     TEXT,
    output_preview    TEXT
);

CREATE INDEX IF NOT EXISTS idx_requests_timestamp ON requests(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_requests_provider  ON requests(provider);
CREATE INDEX IF NOT EXISTS idx_requests_status    ON requests(status);
