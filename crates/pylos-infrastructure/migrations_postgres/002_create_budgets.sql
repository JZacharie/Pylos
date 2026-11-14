CREATE TABLE IF NOT EXISTS vk_budgets (
    virtual_key_id  TEXT    NOT NULL,
    period          TEXT    NOT NULL,
    max_usd         DOUBLE PRECISION NOT NULL,
    current_usd     DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    reset_at        BIGINT  NOT NULL,
    PRIMARY KEY (virtual_key_id, period)
);

CREATE INDEX IF NOT EXISTS idx_vk_budgets_vk ON vk_budgets(virtual_key_id);
