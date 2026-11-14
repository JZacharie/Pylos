# Pylos — Architecture & LLM Context (`CONTEXT.md`)

> **Objectif** : Mémoire structurée du projet **Pylos** (LLM Gateway & MCP Orchestrator en Rust + React) pour permettre à tout développeur ou agent LLM de comprendre immédiatement les modules profonds (*deep modules*), les invariants métier et les points d'extension (*seams*).

---

## 1. Architecture en 4 Crates + UI

| Couche | Crate / Dossier | Responsabilité |
| :--- | :--- | :--- |
| **Domain** | `crates/pylos-core` | Types canoniques (`PylosRequest`, `PylosResponse`, `RequestContext`, `ChatCompletionRequest`, `PylosError`, traits `Provider`, `LlmPlugin`, stores). Zéro dépendance HTTP/DB. |
| **Application** | `crates/pylos-application` | Cas d'usage (`InferenceUseCase`), routeur intelligent (`SmartRouterPlugin`), disponibilité & FinOps (`ProviderAvailabilityService`), garde-fou quotas (`QuotaGuard`), guardrails, cache sémantique/préfixe, boucle agent MCP (`mcp::run_agent_loop`). |
| **Infrastructure** | `crates/pylos-infrastructure` | Adaptateurs providers natifs (`openai`, `anthropic`, `gemini`, `bedrock`, `cohere`, `antigravity`), persistance duale SQLite / PostgreSQL, `ModelCatalog`, export OTEL/OpenObserve. |
| **Interface / Server** | `crates/pylos-server` | Serveur HTTP Axum (`routes.rs`, `state.rs`, `interfaces/http/*`), authentification Admin & Virtual Keys, proxy MCP, compression Caveman, embedding des assets UI (`rust-embed`). |
| **Frontend** | `ui/` | Application React 19 + TypeScript + Vite (`Playground`, `RouterSettings`, `CostTracking`, `Sidebar`, `api.ts`). |

---

## 2. Deep Modules & Seams Principaux

### A. `InferenceUseCase` (`crates/pylos-application/src/use_cases/inference.rs`)
- **Rôle** : Point d'entrée unique de l'inférence (`embed`, `complete`, `stream`).
- **Invariants** :
  1. **Exclusivité périmètre Virtual Key** : `x-pylos-force-provider` / `x-pylos-force-model` ne peuvent jamais élargir les droits d'une clé virtuelle.
  2. **Disponibilité (#207)** : Tout provider marqué `Unavailable` (401/403/402 ou circuit ouvert) ou `Saturated` (429 / cooldown actif) est exclu de `select_ordered_providers`. Si aucun provider autorisé n'est disponible, retourne `PylosError::ServiceUnavailable` (HTTP `503`).
  3. **FinOps Free-First (#208)** : Dans chaque groupe (`supporting` puis `non_supporting`), les providers `BillingTier::Free` et `BillingTier::QuotaSubscription` (Tier 1) sont toujours ordonnés avant `BillingTier::Paid` (Tier 2). Le fallback vers Tier 2 incrémente `FinOpsRoutingStats::free_to_paid_fallbacks`.
  4. **Frontières de raisonnement (#210, #211)** : Lors d'un basculement cross-provider (`last_error.is_some()`), `sanitize_reasoning_boundaries()` nettoie les blocs `reasoning_content` / `<thought>` incompatibles et enregistre le type de fallback (`rate_limit_fallback`, `context_window_fallback`, `error_fallback`) dans `ctx.attempted_fallbacks`.

### B. `ProviderAvailabilityService` (`crates/pylos-application/src/provider_availability.rs`)
- Maintient en mémoire l'état de santé temps réel (`Available`, `Saturated`, `Unavailable`), les fenêtres `Retry-After` / cooldown et le `BillingTier` (`Free`, `QuotaSubscription`, `Paid`) synchronisé depuis `ModelCatalog` et `ProviderConfig`.
- Expose les statistiques FinOps (`GET /v1/routing/finops`) et le statut par provider (`GET /v1/providers`).

### C. `QuotaGuard` (`crates/pylos-application/src/quota_guard.rs`)
- **Rôle (#209, #212)** : Gestion des quotas d'abonnement multi-comptes (ex. `antigravity` via `AGY_ACCOUNT`).
- **Comportement** :
  - Rotation automatique vers le compte ayant la plus grande marge restante avant son `soft_cap`.
  - Si tous les comptes dépassent le `hard_cap`, retourne `QuotaExceeded` et permet le fallback.
  - **Délestage intelligent (`quota_low_shedding`)** : Lorsque la marge globale descend sous `low_threshold_pct` (ou `economy_mode == true`), les requêtes `RequestPriority::Recurring` / `Background` (`x-pylos-priority: recurring`) sont délestées pour préserver le quota pour les requêtes `Interactive`.
  - Endpoints : `GET /v1/quotas`, `POST /v1/quotas/policy`.

### D. `SmartRouterPlugin` (`crates/pylos-application/src/smart_router.rs`)
- **Rôle (#210, #211)** : Pré-analyseur léger + routage 3 tiers (`Flash`, `Mid`, `Frontier`) + compaction de contexte SLM.
- **Thèmes (`TaskTheme`)** : `code`, `reasoning`, `summary`, `vision`, `general` (détectés via `x-pylos-theme`, alias modèle `auto:<theme>` / `pylos/<theme>` ou analyse du prompt).
- **Standardisation `reasoning_effort`** : `low` | `medium` | `high`, systématiquement renseigné et traduit nativement vers OpenAI (`reasoning_effort`), Anthropic (`thinking.budget_tokens`) et Gemini (`generationConfig.thinkingConfig.thinkingBudget`).
- **Escalade automatique** : Via `x-pylos-escalate: true` ou historique `x-pylos-attempted-tiers`, monte directement au tier supérieur sans retomber sur le tier échoué.

---

## 3. En-têtes HTTP Pylos (Observabilité & Contrôle)

| En-tête | Direction | Rôle |
| :--- | :--- | :--- |
| `x-pylos-force-provider` / `x-pylos-force-model` | Requête | Force un provider ou modèle dans le périmètre de la clé. |
| `x-pylos-priority` | Requête | `interactive` (défaut) \| `recurring` \| `background`. |
| `x-pylos-agy-account` | Requête | Cible un compte Antigravity précis. |
| `x-pylos-reasoning-effort` | Req / Resp | `low` \| `medium` \| `high`. |
| `x-pylos-theme` | Req / Resp | `code` \| `reasoning` \| `summary` \| `vision` \| `general`. |
| `x-pylos-escalate` | Requête | `true` pour déclencher une escalade de tier immédiate. |
| `x-pylos-attempted-tiers` | Req / Resp | Liste CSV des tiers déjà tentés (`flash,mid`). |
| `x-pylos-attempted-fallbacks` | Réponse | Trace des fallbacks typés (`rate_limit_fallback:openai`). |

---

## 4. Maintenance de la Documentation & ADR
- Toute PR modifiant l'architecture, un module profond ou un contrat HTTP doit mettre à jour `CONTEXT.md` et ajouter un ADR dans `docs/adr/` si une décision structurante est prise.
