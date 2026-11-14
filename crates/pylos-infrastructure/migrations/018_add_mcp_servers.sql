CREATE TABLE IF NOT EXISTS mcp_servers (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    server_type       TEXT NOT NULL DEFAULT 'python',
    status            TEXT NOT NULL DEFAULT 'inactive',
    target_url        TEXT,
    container_image   TEXT,
    env_vars          TEXT, -- JSON object of environment variables (encrypted values)
    virtual_key_id    TEXT REFERENCES virtual_keys(id) ON DELETE SET NULL,
    team_id           TEXT,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mcp_servers_vk ON mcp_servers(virtual_key_id);
CREATE INDEX IF NOT EXISTS idx_mcp_servers_team ON mcp_servers(team_id);
CREATE INDEX IF NOT EXISTS idx_mcp_servers_status ON mcp_servers(status);
