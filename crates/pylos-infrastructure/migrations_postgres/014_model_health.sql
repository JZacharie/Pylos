CREATE TABLE IF NOT EXISTS model_health (
    id             TEXT PRIMARY KEY, -- provider/model_id
    provider       TEXT NOT NULL,
    model_id       TEXT NOT NULL,
    health_status  TEXT NOT NULL DEFAULT 'none',
    error_details  TEXT,
    last_check_ms  INTEGER,
    last_success_ms INTEGER
);
