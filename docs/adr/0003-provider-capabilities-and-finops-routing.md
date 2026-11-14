# ADR 0003 — Matrice de Capacités Providers & Routage FinOps / Quotas

- **Statut** : Accepté (2026-11-14)
- **Issues liées** : `#140`, `#207`, `#208`, `#209`, `#210`, `#211`, `#212`

## Contexte
Pylos fédère plus de 20 providers (natifs + compatibles OpenAI + abonnement `antigravity`). Sans capacités normalisées ni hiérarchie de coûts/santé, le routeur pouvait envoyer `tools` à des providers incompatibles ou router vers des APIs payantes alors qu'un quota gratuit ou d'abonnement était disponible.

## Décision
1. **`ProviderKind::capabilities()` (`pylos-core`)** : Chaque provider déclare `ProviderCapabilities { chat, streaming, tools, vision, embeddings, images }`. Le routage filtre automatiquement les providers incompatibles avant l'envoi.
2. **Santé & Disponibilité (`ProviderAvailabilityService`)** : Les providers `Unavailable` (401/403/402/circuit ouvert) ou `Saturated` (429/cooldown) sont exclus immédiatement ; si aucun provider autorisé n'est disponible, l'API renvoie HTTP `503 ServiceUnavailable`.
3. **FinOps Free-First & `QuotaGuard`** : Le Tier 1 (`Free` + `QuotaSubscription`) est toujours priorisé avant le Tier 2 (`Paid`), avec rotation multi-comptes Antigravity (`AGY_ACCOUNT`) et délestage des tâches `Recurring` sous seuil bas (`quota_low_shedding`).
4. **Standardisation `reasoning_effort`** : `low` / `medium` / `high` traduit vers OpenAI, Anthropic (`thinking.budget_tokens`) et Gemini (`thinkingConfig.thinkingBudget`), avec nettoyage des frontières de raisonnement cross-provider.
