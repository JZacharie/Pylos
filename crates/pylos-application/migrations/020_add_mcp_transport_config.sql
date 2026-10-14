-- Migration 020: Add transport_config column to mcp_servers table (SQLite)
ALTER TABLE mcp_servers ADD COLUMN transport_config TEXT;
