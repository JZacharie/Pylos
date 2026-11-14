# Matrice Fonctionnalités & Capacités Providers (README ↔ Code)

> Référence alignée sur l'état réel du code (`#137`, `#140`, `#141`).

## 1. Statut des Fonctionnalités

| Fonctionnalité | Statut | Détails & Implémentation |
| :--- | :--- | :--- |
| **Chat / Streaming SSE** | Stable | `crates/pylos-server/src/interfaces/http/inference.rs` |
| **Fallback & Circuit Breaker (#207)** | Stable | `ProviderAvailabilityService` (HTTP 503 si tous indisponibles) |
| **Routage FinOps Free-First (#208)** | Stable | Tier 1 (`Free` / `QuotaSubscription`) avant Tier 2 (`Paid`) |
| **Antigravity Multi-Comptes & QuotaGuard (#209, #212)** | Stable | `QuotaGuard`, `AGY_ACCOUNT`, `GET /v1/quotas`, `quota_low_shedding` |
| **SmartRouter & `reasoning_effort` (#210, #211)** | Stable | Pré-analyseur thème/complexité, escalade auto, nettoyage cross-provider |
| **Guardrails & PII Masking** | Stable | Mode bloquant / moniteur, normalisation Unicode anti-évasion |
| **MCP Gateway & Agent Loop** | Stable | SSRF guard, gouvernance par Virtual Key, streaming tool calls |
| **RAG + Reranking + Citations (#138)** | Beta | `RagPlugin` (Qdrant + HyDE + CRAG + reranking hybride + `Recall@K`) |
| **Cross-Agent Memory Graph (Memgraph) (#137)** | Optionnel | Feature Cargo `memgraph` (désactivée par défaut, vérifiée en CI via `--features memgraph`) |
| **Observabilité Prometheus / OpenObserve (#139, #214)** | Stable | Métriques tenant/coût, audit admin, dashboards Grafana & OpenObserve v8 |

## 2. Matrice de Capacités par Provider (`ProviderKind::capabilities()`)

| Provider | Chat | Streaming | Tools | Vision | Embeddings | Images |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `openai`, `azure` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `gemini`, `vertex` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `anthropic` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `bedrock`, `openrouter`, `mistral`, `ollama-jo3` | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| `cohere` | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ |
| `groq`, `fireworks`, `xai`, `antigravity` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `deepseek`, `nebius`, `lemonade` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `cerebras`, `perplexity` | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
