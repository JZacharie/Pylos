# ADR 0002: Persistence Ports and Unified SQL Strategy

- **Status**: Accepted
- **Date**: 2026-11-14
- **Issues**: #127, #128

## Context

Previously, `pylos-application` directly depended on `sqlx` and `rusqlite`, coupling application use cases and plugins (`BudgetPlugin`, `RateLimitPlugin`, `ConfigStore`) to concrete database drivers. Furthermore, every persistence store duplicated SQL logic across `match DbPool::Sqlite(_) | DbPool::Postgres(_)` branches, increasing maintenance cost and risking behavioral drift between SQLite and PostgreSQL.

## Decision

### 1. Hexagonal Persistence Ports (`#127`)
- **Domain Ports (`pylos_core::domain::repositories`)**: Defined async repository traits (`BudgetRepository`, `RateLimitRepository`, `ConfigRepository`, `VirtualKeyRepository`, `LogRepository`, `OrganizationRepository`) and pure domain logging types (`pylos_core::domain::logging`).
- **Zero DB Dependencies in Core & Application**: Neither `pylos-core` nor `pylos-application` depends on `sqlx` or `rusqlite`. Application services (`ConfigStore`, `BudgetPlugin`, `RateLimitPlugin`) depend only on `Arc<dyn *Repository>`.
- **In-Memory Fakes**: Provided `InMemory*Repository` implementations in `pylos_core::domain::repositories` for fast, DB-free unit tests in `pylos-application`.
- **Infrastructure Adapters (`pylos_infrastructure::persistence`)**: Moved all SQL stores and migrations (`migrations/`, `migrations_postgres/`) into `pylos-infrastructure` and implemented the domain repository traits.

### 2. Unified SQL Portability (`#128`)
- **Unified `DbPool` / `DbTx` / `DbRow` / `DbValue`**: Introduced a dialect-aware abstraction in `pylos_infrastructure::persistence::db_pool` that translates positional placeholders (`?` -> `$1, $2, ...` for Postgres) and binds/reads typed values (`Text`, `OptText`, `Int`, `OptInt`, `Float`, `Bool`, `OptJson`) uniformly across SQLite and PostgreSQL.
- **Single-Path Stores**: Replaced duplicated `match DbPool::Sqlite/Postgres` query execution blocks in `BudgetStore`, `RateLimitStore`, `VirtualKeyStore`, `OrganizationStore`, `SystemPromptStore`, `SearchToolStore`, `McpServerStore`, `ModelCatalog`, `SqlConfigRepository`, and `SqlLogStore` (`PgLogStore`/`SqliteLogStore` aliases).
- **Schema Migrations**: Dialect-specific DDL remains isolated in `migrations/` (SQLite) and `migrations_postgres/` (PostgreSQL), applied at pool initialization.

## Consequences

- `pylos-core` and `pylos-application` compile and test without `sqlx` or `rusqlite`.
- Store behavior is shared across SQLite and PostgreSQL through `DbPool`, eliminating duplicate query/mapping code.
- Integration and dual-backend tests validate SQLite in-memory by default and optionally PostgreSQL when `PYLOS_TEST_POSTGRES_URL` is configured.
