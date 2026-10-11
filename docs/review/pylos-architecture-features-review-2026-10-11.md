# Revue d'architecture & de fonctionnalités — Pylos

> **Commit de référence** : `2310d68f` (`main`, 2026-10-11 — après les 11 merges de dépendances).
> **Périmètre** : architecture du workspace Rust, système de plugins, providers, persistance, console React, chaîne de déploiement, couverture fonctionnelle annoncée vs réelle.
> **Documents liés** : [revue de sécurité du 11/10/2026](pylos-code-review-2026-10-11.md).
> **Suivi** : le plan d'action de ce document est suivi dans GitHub — **4 jalons** et **30 issues** (#112 → #141).
> **Auteur** : agent d'audit (DeepSeek Harness), sur demande de J. Zacharie.

---

## 1. Synthèse

Pylos est une passerelle LLM multi-providers en Rust, structurée en workspace de 4 crates et enrichie d'une console React de 38 pages. Le socle est **crédible et bien outillé** (clippy `-D warnings`, 103 tests, CI multi-arch + multi-stage Docker, chart Helm, CodeQL/MegaLinter) et couvre un périmètre fonctionnel inhabituellement large pour un projet de cette taille : routage pondéré et round-robin, budgets/Virtual Keys, garde-fous, cache préfixe et sémantique, RAG, batching, compression de contexte, proxy MCP, multi-tenant.

Trois limites structurelles ressortent :

1. **L'architecture « ports & adapters » n'est vraie qu'à moitié** : si les providers et les plugins sont bien derrière des traits, la persistance est en revanche directement codée dans la couche application (`sqlx` + stores concrets), avec une duplication SQLite/Postgres systématique. C'est la principale dette d'architecture.
2. **Le multi-tenant et la gouvernance ne sont pas étanches** : plusieurs chemins de contournement (cache non scopé, budgets câblés sur le mauvais identifiant, restrictions de provider ignorées, en-têtes client souverains) font que les promesses du README ne sont pas garanties par le code.
3. **L'écart documentation ↔ réalité** est important : « 20+ providers », « fallback automatique », « block unsafe content », « graphe mémoire », « < 2 ms d'overhead » sont vrais partiellement, ou sous conditions (feature désactivée, mode non bloquant, non mesuré).

Le projet reste donc **non prêt pour la production**, ce que le README assume — mais la feuille de route ci-dessous (P0 → P3, 4 sprints) permet d'y arriver.

---

## 2. Architecture observée

### 2.1 Découpage et sens des dépendances

```text
pylos-core           domaine : entités, traits (Provider, LlmPlugin), config, erreurs
     ▲
pylos-infrastructure adaptateurs providers (openai, anthropic, gemini, cohere, bedrock, azure)
     ▲
pylos-application    cas d'usage, orchestration, stores, plugins, guardrails, RAG, cache
     ▲
pylos-server         Axum : routes, middlewares, état applicatif, MCP, UI embarquée (rust-embed)
```

**Points forts** : sens de dépendance globalement respecté ; `pylos-core` ne dépend ni d'Axum ni de sqlx ; orchestration isolée dans `use_cases/inference.rs` ; plugins et providers derrière des traits ; `AppState` centralise l'injection.

**Limites** :
- `pylos-application` dépend de `sqlx`, `rusqlite` et `DbPool` et expose des stores concrets → il n'y a **pas de port de persistance**.
- Chaque store réimplémente deux dialectes SQL (`match &self.pool { Sqlite => …, Postgres => … }`) : `log_store.rs` + `sqlite_log_store.rs` + `pg_log_store.rs`, `budget_store`, `rate_limit_store`, `virtual_key_store`, `organization_store`, `model_catalog`, `mcp_server_store`… Le risque de divergence fonctionnelle entre backends est structurel.
- `pylos-server` contient de la logique métier (remapping de modèles, agrégation de logs, MCP proxy, compression Caveman), au-delà de son rôle d'adaptateur HTTP.

### 2.2 Chaîne de traitement d'une requête

```text
HTTP → request_id → CORS → [management_auth | virtual_key] → playgroup → queuing (sémaphore + timeout)
     → InferenceOrchestrator.complete/stream
         pre_hooks (ordre d'enregistrement) : cache_aligner, budget, structured_output,
                                              rate_limit, rag, otel, memory, guardrails,
                                              mem0, prefix_cache, batching, smart_router
         routage : filtrage VK → supporting/non-supporting → circuit breaker → round-robin → weighted shuffle
         providers : retry + backoff exponentiel avec jitter → fallback
         post_hooks (LIFO) → réponse
     → métriques Prometheus + log store (SQLite/Postgres) + OTel
```

Fragilités relevées :
- **Ordre des plugins implicite** (ordre d'enregistrement) et post-hooks LIFO non documentés → comportement difficile à raisonner et à tester.
- **Post-hooks inopérants en streaming** : la réponse reconstruite est jetée, le client reçoit les chunks bruts (démasquage PII, structured output, comptage budgétaire non appliqués).
- **Circuit breaker** : compteur par provider en mémoire, remis à zéro au redémarrage, sans état half-open explicite ; les providers « ouverts » sont tout de même essayés en fin de chaîne.
- **Fallback** : limité aux `Timeout`, ce qui neutralise la promesse de résilience.

### 2.3 Providers

|
| Niveau | Providers |
|---|---|
| Adaptateurs natifs | OpenAI, Anthropic, Gemini, Cohere, Bedrock, Azure |
| OpenAI-compatibles (URL/API standard) | Groq, Mistral, Cerebras, Perplexity, Fireworks, xAI, Nebius, DeepSeek, OpenRouter, Lemonade, Ollama, Vertex |
| Custom | URL de base libre (`ProviderKind::Custom`) |

Le README annonce « 20+ providers » ; le code implémente **6 adaptateurs natifs** + un adaptateur générique. Les capacités (outils, vision, embeddings, images) ne sont pas déclarées par provider ni validées au routage.

### 2.4 Persistance

| Aspect | État |
|---|---|
| Backend par défaut | SQLite (fichiers `pylos-*.db`) |
| Backend alternatif | PostgreSQL (mêmes stores, code dupliqué) |
| Config | table `gateway_config` : JSON complet, **sans numéro de version ni migration**, clés providers en clair |
| VK | valeur chiffrée AES-256-GCM, lookup par `value_hash` SHA-256 |
| Multi-répliques | non sûr avec SQLite + registries en mémoire (Helm déploie 2 réplicas) |

### 2.5 Console React

38 pages (Dashboard, Logs, VirtualKeys, ModelCatalog, Providers, Guardrails, VectorStores, MCP, Playground, Access management…), Vite 8 + React 19 + Tailwind. **Aucun test front**, **aucun lint dans la CI**, un build UI validé uniquement sur push `main` (et il était cassé jusqu'au 11/10). Les fichiers `ModelCatalog.tsx` (~84 Ko), `Playground.tsx`, `VirtualKeys.tsx` concentrent la complexité.

### 2.6 Exploitation

- **CI** : `Check` (clippy + tests), `MegaLinter`, `CodeQL`, build multi-arch, manifest merge, déploiement dev sur k8s.
- **CD** : build & push GHCR (backend + UI), `kubectl rollout restart` sur `pylos-dev`.
- **Branche `main` protégée** : force push/suppression interdits, 3 checks requis + branche à jour.
- Manquent : benchmarks, SIT, tests de contrat providers, journals d'audit admin, dashboards versionnés, HPA/PDB/NetworkPolicy, stratégie de migration de configuration.

---

## 3. Matrice de vérité fonctionnelle (README ↔ code)

| Fonctionnalité annoncée | Réalité code | Statut |
|---|---|---|
| Passerelle OpenAI-compatible (`/v1/chat/completions`, `/v1/embeddings`, `/v1/images/generations`) | Implémentée | ✅ Stable |
| 20+ providers | 6 natifs + adaptateur compatible | ⚠️ À préciser |
| Routage pondéré, round-robin, règles CEL | Pondéré + RR implémentés ; pas de trace d'un évaluateur CEL | ⚠️ Partiel |
| Fallback multi-provider automatique | Uniquement sur `Timeout` | ⚠️ Partiel |
| Circuit breaking + retries exponentiels | Présents (mémoire locale, pas de half-open) | 🟡 Beta |
| Budgets mensuels/hebdomadaires | Non appliqués (câblage id/alias cassé) | ❌ Inopérant |
| Rate limiting RPM/TPM par Virtual Key | RPM en mémoire par processus, TPM via store persistant | ⚠️ Divergent |
| Garde-fous PII/blocage de contenu | Masquage + détection **non bloquante**, bypass par en-tête client | ⚠️ Partiel |
| Journalisation SQLite/Postgres + histogrammes | Implémentée ; PII conservées en clair | ✅/⚠️ |
| Cache préfixe TinyLFU | Implémenté ; **non scopé par tenant** | ⚠️ Risque |
| Cache sémantique Qdrant | Implémenté ; **sans filtre de locataire**, dépendance Qdrant externe | ⚠️ Risque |
| Cross-Agent Memory Graph (Memgraph) | Feature `memgraph` **désactivée par défaut** | ❌ Optionnel |
| RAG automatique multi-collections | Implémenté (embedding, expansion, HyDE, CRAG partiel) ; pas de reranking/citations | 🟡 Beta |
| MCP proxy + serveurs MCP dynamiques | Implémenté ; endpoints de statut non authentifiés, en-têtes fuités à l'amont | ⚠️ Risque |
| Batching (coalescing + OpenAI Batch API) | Implémenté | ✅ Stable |
| Compression de contexte (Caveman, aligner de cache) | Implémenté | 🟡 Beta |
| Multi-tenancy (Organisations, Équipes, VK scopées) | Modèle présent ; étanchéité non garantie (cache, management API) | ⚠️ Partiel |
| OAuth Google + clé admin | Implémenté ; pas de `state`, premier inscrit = admin | ⚠️ Risque |
| Observabilité Prometheus/OTel | Présente ; pas de métriques par tenant, pas d'audit admin | 🟡 Beta |
| « < 2 ms d'overhead » | Non mesuré (`opt-level = "z"`) | ❌ Non prouvé |
| Hot-reload de configuration | Implémenté (reload + hash), schéma non versionné | 🟡 Beta |

---

## 4. Constats architecture & fonctionnalités

### Bloquants (P0)
| # | Constat | Réf. |
|---|---|---|
| A-01 | Toute Virtual Key donne accès à l'API de management | #112 |
| A-02 | Aucun contrôle de rôle sur les JWT de session | #113 |
| A-03 | Contournement guardrails + restrictions provider par en-têtes client | #114 |
| A-04 | PII/secrets journalisés et persistés en clair | #115 |
| A-05 | PAT GitHub en clair dans la configuration locale | #116 |
| A-06 | Budgets inopérants (alias vs identifiant de VK) | #117 |
| A-07 | Fallback multi-provider limité aux timeouts | #118 |

### Élevés / moyens (P1)
| # | Constat | Réf. |
|---|---|---|
| A-08 | Mot de passe admin en SHA-256 non salé | #119 |
| A-09 | Clé de chiffrement éphémère, loguée, en `thread_local` | #120 |
| A-10 | `expires_at` des VK jamais appliqué | #121 |
| A-11 | Clés providers stockées en clair | #122 |
| A-12 | OAuth sans `state` + auto-provisionnement + premier inscrit admin | #123 |
| A-13 | `/mcp/{server}` et `/metrics` non authentifiés ; en-têtes fuités à l'amont MCP | #124 |
| A-14 | **Caches non cloisonnés par tenant** (préfixe et sémantique) — fuite inter-locataires | #125 |
| A-15 | Rate limiting non distribué, deux mécanismes divergents | #126 |

### Dette d'architecture (P2)
| # | Constat | Réf. |
|---|---|---|
| A-16 | Absence de ports de persistance (application couplée à sqlx) | #127 |
| A-17 | Duplication SQLite/Postgres dans tous les stores | #128 |
| A-18 | Pipeline de plugins implicite + post-hooks streaming inopérants | #129 |
| A-19 | 211 `unwrap`/`panic`-`abort` dans le code de production | #130 |
| A-20 | Config `gateway_config` sans version ni migration | #131 |
| A-21 | Absence de SIT et de tests de contrat providers (103 tests unitaires/intégration) | #132 |
| A-22 | Aucun benchmark ; objectif de latence non prouvé ; `opt-level = "z"` | #133 |
| A-23 | Front non testé, non linté en CI, pages monolithiques | #134 |
| A-24 | Garde-fous non bloquants malgré la promesse produit | #135 |

### Améliorations (P3)
| # | Constat | Réf. |
|---|---|---|
| A-25 | Chart Helm multi-répliques incohérent avec SQLite/registries mémoire ; pas de HPA/PDB/NetworkPolicy | #136 |
| A-26 | Memgraph désactivé par défaut malgré la mise en avant README | #137 |
| A-27 | RAG sans reranking, citations ni évaluation | #138 |
| A-28 | Observabilité sans dimension locataire ni audit admin | #139 |
| A-29 | Pas de matrice de capacités providers | #140 |
| A-30 | Pas d'ADR, pas de matrice README↔code, pas de runbook | #141 |

---

## 5. Plan d'action et planning

Le suivi est assuré dans GitHub : **4 jalons** (#1 → #4) et **30 issues** (#112 → #141), étiquetées `scope:*`, `type:*` et `priority:p0..p3`.

| Sprint | Jalons | Échéance | Objectif | Issues |
|---|---|---|---|---|
| **Sprint 0 — Sécurité bloquante** | M#1 | **26/10/2026** | Rendre le plan de contrôle sûr et la gouvernance effective avant toute exposition | [#112](https://github.com/JZacharie/Pylos/issues/112) · [#113](https://github.com/JZacharie/Pylos/issues/113) · [#114](https://github.com/JZacharie/Pylos/issues/114) · [#115](https://github.com/JZacharie/Pylos/issues/115) · [#116](https://github.com/JZacharie/Pylos/issues/116) · [#117](https://github.com/JZacharie/Pylos/issues/117) · [#118](https://github.com/JZacharie/Pylos/issues/118) |
| **Sprint 1 — Gouvernance & fiabilité** | M#2 | **23/11/2026** | Chiffrement, OAuth, isolation multi-tenant, rate limiting distribué | [#119](https://github.com/JZacharie/Pylos/issues/119) · [#120](https://github.com/JZacharie/Pylos/issues/120) · [#121](https://github.com/JZacharie/Pylos/issues/121) · [#122](https://github.com/JZacharie/Pylos/issues/122) · [#123](https://github.com/JZacharie/Pylos/issues/123) · [#124](https://github.com/JZacharie/Pylos/issues/124) · [#125](https://github.com/JZacharie/Pylos/issues/125) · [#126](https://github.com/JZacharie/Pylos/issues/126) |
| **Sprint 2 — Architecture & qualité** | M#3 | **21/12/2026** | Ports/adapters, suppression de duplication, pipeline de plugins, tests système, benchmarks | [#127](https://github.com/JZacharie/Pylos/issues/127) · [#128](https://github.com/JZacharie/Pylos/issues/128) · [#129](https://github.com/JZacharie/Pylos/issues/129) · [#130](https://github.com/JZacharie/Pylos/issues/130) · [#131](https://github.com/JZacharie/Pylos/issues/131) · [#132](https://github.com/JZacharie/Pylos/issues/132) · [#133](https://github.com/JZacharie/Pylos/issues/133) · [#134](https://github.com/JZacharie/Pylos/issues/134) · [#135](https://github.com/JZacharie/Pylos/issues/135) |
| **Sprint 3 — Fonctionnalités & observabilité** | M#4 | **15/02/2027** | Memgraph, RAG avancé, observabilité multi-tenant, matrice providers, documentation | [#136](https://github.com/JZacharie/Pylos/issues/136) · [#137](https://github.com/JZacharie/Pylos/issues/137) · [#138](https://github.com/JZacharie/Pylos/issues/138) · [#139](https://github.com/JZacharie/Pylos/issues/139) · [#140](https://github.com/JZacharie/Pylos/issues/140) · [#141](https://github.com/JZacharie/Pylos/issues/141) |

### Définition de « terminé » (commune)
- Le comportement est couvert par un test positif **et** un test négatif (sécurité) ou un test d'intégration (fonctionnel).
- `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` verts.
- La documentation (README, `docs/`, ADR) est mise à jour dans le même changement.
- Toute affirmation produit touchée est soit rendue vraie, soit corrigée dans le README.

### Ordre de traitement recommandé
1. **Sprint 0** intégralement (bloquant, ~2 semaines).
2. Puis P1 par lots thématiques : chiffrement/secrets (#119, #120, #122), identité (#121, #123), exposition réseau (#124), multi-tenant (#125, #126).
3. En parallèle du Sprint 1, démarrer #127/#128 (chantier long) et #132 (SIT) pour ne pas décaler le Sprint 2.
4. P3 ensuite, en priorisant la valeur perçue : #139 (observabilité), #138 (RAG), #137 (mémoire).

### Conseils de pilotage
- **Sprints de 2 semaines** avec revue de jalon à l'échéance ; les issues P0 non terminées bloquent l'ouverture externe.
- **Un chantier = une PR** avec la référence `Closes #NNN`, la protection de branche exigeant les 3 checks.
- Ajouter un **job CI « UI »** (`npm ci && npm run build && npm run lint`) : sans lui, aucune régression front n'est détectée (#134).
- Un tableau **GitHub Projects** serait un complément utile, mais l'API Projects v2 nécessite un token avec le scope `project` (le token actuel ne l'a pas).

---

## 6. Méthode et limites

- **Méthode** : lecture du code des 4 crates et de la console, inventaire des routes/plugins/providers/stores, confrontation README ↔ implémentation, vérification par `cargo check`, `cargo clippy -D warnings`, `cargo test` (103 tests), `npm ci` + `npm run build`, et analyse de l'historique CI.
- **Limites** : pas de tests dynamiques d'intrusion ni de charge (SIT absents) ; les constats de scalabilité multi-répliques (#126, #136) sont issus de la lecture du code et du chart, non reproduits sur un cluster ; la revue n'est pas un audit exhaustif ligne à ligne de la console React.

---

*Document généré le 11/10/2026 à partir du commit `2310d68f`. Plan d'action suivi dans GitHub : [issues](https://github.com/JZacharie/Pylos/issues) · [jalons](https://github.com/JZacharie/Pylos/milestones).*
