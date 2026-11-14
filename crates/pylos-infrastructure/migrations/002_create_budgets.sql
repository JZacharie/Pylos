-- Migration 002: table de budget par virtual key
-- Remplace le BudgetConfig in-memory de la config
-- Bifrost source: plugins/governance/budget.go

CREATE TABLE IF NOT EXISTS vk_budgets (
    virtual_key_id  TEXT    NOT NULL,
    period          TEXT    NOT NULL,  -- "daily" | "monthly" | "total"
    max_usd         REAL    NOT NULL,
    current_usd     REAL    NOT NULL DEFAULT 0.0,
    reset_at        INTEGER NOT NULL,  -- Unix timestamp ms quand le budget se reset
    PRIMARY KEY (virtual_key_id, period)
);

CREATE INDEX IF NOT EXISTS idx_vk_budgets_vk ON vk_budgets(virtual_key_id);
