-- Migration 022: Add transport_config column to mcp_servers table (PostgreSQL)
ALTER TABLE mcp_servers ADD COLUMN IF NOT EXISTS transport_config TEXT;
