-- Migration 020: Fix Postgres schema types and add missing guardrail columns to requests table
-- Alter model_catalog columns from SMALLINT to BOOLEAN to match Rust models and query bindings
ALTER TABLE model_catalog ALTER COLUMN supports_vision TYPE BOOLEAN USING (supports_vision != 0);
ALTER TABLE model_catalog ALTER COLUMN supports_tools TYPE BOOLEAN USING (supports_tools != 0);
ALTER TABLE model_catalog ALTER COLUMN supports_streaming TYPE BOOLEAN USING (supports_streaming != 0);
ALTER TABLE model_catalog ALTER COLUMN supports_embeddings TYPE BOOLEAN USING (supports_embeddings != 0);
ALTER TABLE model_catalog ALTER COLUMN is_deprecated TYPE BOOLEAN USING (is_deprecated != 0);
ALTER TABLE model_catalog ALTER COLUMN enabled TYPE BOOLEAN USING (enabled != 0);

ALTER TABLE model_catalog ALTER COLUMN supports_vision SET DEFAULT FALSE;
ALTER TABLE model_catalog ALTER COLUMN supports_tools SET DEFAULT TRUE;
ALTER TABLE model_catalog ALTER COLUMN supports_streaming SET DEFAULT TRUE;
ALTER TABLE model_catalog ALTER COLUMN supports_embeddings SET DEFAULT FALSE;
ALTER TABLE model_catalog ALTER COLUMN is_deprecated SET DEFAULT FALSE;
ALTER TABLE model_catalog ALTER COLUMN enabled SET DEFAULT TRUE;

-- Add guardrail columns to requests table if they do not exist
ALTER TABLE requests ADD COLUMN IF NOT EXISTS guardrail_triggered BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE requests ADD COLUMN IF NOT EXISTS guardrail_type TEXT;
ALTER TABLE requests ADD COLUMN IF NOT EXISTS guardrail_detail TEXT;
