ALTER TABLE virtual_keys ADD COLUMN value_hash TEXT;
CREATE INDEX IF NOT EXISTS idx_vk_value_hash ON virtual_keys(value_hash);
