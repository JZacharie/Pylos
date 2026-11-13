-- Migration 021: Ajout de allowed_tools et denied_tools pour la gouvernance MCP (S-MCP-12)
ALTER TABLE virtual_keys ADD COLUMN allowed_tools TEXT;
ALTER TABLE virtual_keys ADD COLUMN denied_tools TEXT;
