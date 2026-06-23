CREATE TABLE IF NOT EXISTS search_tools (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    tool_type   TEXT NOT NULL,
    config      JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  BIGINT NOT NULL,
    updated_at  BIGINT NOT NULL
);
