-- Migration 020: Drop SMALLINT defaults before type change in 021
-- Must be in its own migration (separate transaction) because PG cannot cast
-- SMALLINT default 0 to BOOLEAN when using ALTER COLUMN TYPE

ALTER TABLE model_catalog ALTER COLUMN supports_vision DROP DEFAULT;
ALTER TABLE model_catalog ALTER COLUMN supports_tools DROP DEFAULT;
ALTER TABLE model_catalog ALTER COLUMN supports_streaming DROP DEFAULT;
ALTER TABLE model_catalog ALTER COLUMN supports_embeddings DROP DEFAULT;
ALTER TABLE model_catalog ALTER COLUMN is_deprecated DROP DEFAULT;
ALTER TABLE model_catalog ALTER COLUMN enabled DROP DEFAULT;
