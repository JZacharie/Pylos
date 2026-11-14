CREATE TABLE IF NOT EXISTS pricing_reload_status (
    id                     INT PRIMARY KEY,
    source_url             TEXT NOT NULL,
    last_reload_ms         BIGINT,
    models_count           INT NOT NULL DEFAULT 0,
    periodic_schedule      TEXT
);

INSERT INTO pricing_reload_status (id, source_url, last_reload_ms, models_count, periodic_schedule)
VALUES (1, 'https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json', NULL, 0, NULL)
ON CONFLICT(id) DO NOTHING;
