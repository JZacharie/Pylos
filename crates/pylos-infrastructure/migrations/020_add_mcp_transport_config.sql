-- Migration 020: Ajout du champ transport_config pour McpServer (S-MCP-00)
ALTER TABLE mcp_servers ADD COLUMN transport_config TEXT;
