-- Migration 023: Ajout de allowed_tools et denied_tools pour la gouvernance MCP (S-MCP-12)
ALTER TABLE virtual_keys ADD COLUMN IF NOT EXISTS allowed_tools JSONB;
ALTER TABLE virtual_keys ADD COLUMN IF NOT EXISTS denied_tools JSONB;
