# Plan d'action — backlog historique & MCP Tool Execution Gateway

> **Date** : 11/10/2026 · **Commit de référence** : `e0c6d37` (`main`)
> **Objet** : traiter les 5 issues ouvertes du backlog fonctionnel (#44, #45, #46, #47, #62) et donner un plan d'exécution daté.
> **Suivi GitHub** : jalon **#5 « MCP Tool Execution Gateway - Phase 1 »** (échéance 29/11/2026, epics #142 → #148) et jalon **#6 « Observabilite pipeline Mnemosyne »** (échéance 17/01/2027, issue #47).
> **Documents liés** : [revue de sécurité](pylos-code-review-2026-10-11.md) · [revue architecture & fonctionnalités](pylos-architecture-features-review-2026-10-11.md).

---

## 1. Triage des 5 issues ouvertes

| Issue | Titre | État réel dans le code (vérifié le 11/10) | Conclusion |
|---|---|---|---|
| [#44](https://github.com/JZacharie/Pylos/issues/44) | Setup interactif du mot de passe admin | **Implémenté** : `ui/src/pages/Setup.tsx` (double saisie, ≥ 10 caractères), `GET /api/setup/status` + `POST /api/setup` (`routes/setup.rs`), hash persisté, garde `setup_required` dans le middleware de management | **À fermer** (doublon de #39) — le durcissement du hash reste suivi par #119 |
| [#45](https://github.com/JZacharie/Pylos/issues/45) | Ollama local + auto-pull gemma:2b | **Implémenté** : profil `ollama` et service `ollama-pull` (`docker-compose.yml:78-110`), provider mappé dans `config_store.rs` | **À fermer** (doublon de #40) |
| [#46](https://github.com/JZacharie/Pylos/issues/46) | Routage externe LiteLLM | **Supporté par configuration** : provider OpenAI-compatible + `network.base_url` / `OPENAI_BASE_URL` ; **pas de provider natif ni de recette documentée** | **À fermer après** ajout d'une recette LiteLLM dans `docs/pylos-config.md` (couvert par #141) |
| [#47](https://github.com/JZacharie/Pylos/issues/47) | Observabilité pipeline Mnemosyne | **Non implémenté** — Pylos ne consomme pas l'API pipeline de Mnemosyne (qui expose pourtant `/api/pipeline/runs`, `/runs/{id}`, `/stats`, `/retry`) | **À conserver et planifier** (section 3) |
| [#62](https://github.com/JZacharie/Pylos/issues/62) | MCP Tool Execution Gateway | **Non implémenté** : pas de `transport_config`, pas de MCP client, pas de `/mcp/tools/list`, pas d'agent loop ; aucune branche `feature/s-mcp-00-*` sur le remote | **À exécuter** (section 2) |

**Recommandation de nettoyage du backlog** : fermer #44, #45 et #46 (leurs jumelles #39, #40, #41 sont déjà fermées) après les commentaires de vérification, et conserver #47 et #62 comme issues canoniques. Cela fait passer le compteur d'issues ouvertes de 5 à 2 pour ce filtre.

---

## 2. Plan d'exécution — MCP Tool Execution Gateway (#62)

Le plan BMAD existant (7 epics, 16 stories, 28 points) est **validé**. Il est désormais tracé dans GitHub : jalon **#5**, epics [#142](https://github.com/JZacharie/Pylos/issues/142) → [#148](https://github.com/JZacharie/Pylos/issues/148).

### 2.1 Découpage

| Sprint | Stories | Points | Objectif de sortie |
|---|---|---|---|
| **A** (1,5 sem.) | S-MCP-00 → 03, S-MCP-14 | 10 | Pylos se connecte à un MCP server (stdio/HTTP/SSE) et découvre ses tools ; mock server disponible |
| **B** (1,5 sem.) | S-MCP-05, 04, 06, 07, 10, 09 | 12 | Flow complet : requête → tools → `tool_calls` → exécution → LLM → réponse |
| **C** (1 sem.) | S-MCP-08, 11, 12, 13, 15 | 7 | Streaming, `web_fetch`, gouvernance par Virtual Key, tests end-to-end |

Chemin critique : `S-MCP-00 → S-MCP-01/02 → S-MCP-03 → S-MCP-06 → S-MCP-09 → S-MCP-15`.
Parallélisables : S-MCP-14 (mock), S-MCP-11 (`web_fetch`), S-MCP-12 (modèle de gouvernance).

### 2.2 Décisions de conception recommandées

1. **Réutiliser l'existant plutôt que créer un second modèle** : les `ToolPolicy` (allowed models/providers, quotas) et les `AccessGroup` existent déjà — la gouvernance des tools doit s'y adosser ([EPIC-MCP-06](https://github.com/JZacharie/Pylos/issues/147)).
2. **Trait `ToolExecutor` unifié** (tools MCP + tools built-in) pour que l'agent loop ne connaisse qu'une interface.
3. **Transport** : `transport_config: Option<McpTransportConfig>` nullable → rétro-compatibilité avec les serveurs existants en mode proxy HTTP (`target_url`).
4. **Agent loop dans `pylos-application`**, pas dans le handler HTTP, pour rester testable et réutilisable (streaming compris).
5. **`web_fetch` : protection SSRF obligatoire** (schémas autorisés, blocage loopback/privé/link-local, taille max, timeout, redirections contrôlées).

### 2.3 Prérequis et risques à traiter en parallèle

| Risque | Issue liée | Impact sur le MCP |
|---|---|---|
| Endpoints MCP non authentifiés + `Authorization` transmis aux serveurs amont | [#124](https://github.com/JZacharie/Pylos/issues/124) | Fuite de la clé de l'appelant vers les MCP servers ; à corriger **avant** le Sprint A |
| Post-hooks inopérants en streaming | [#129](https://github.com/JZacharie/Pylos/issues/129) | `S-MCP-08` (agent loop streaming) ne pourra pas appliquer governance/budget/PII |
| Caches non cloisonnés par tenant | [#125](https://github.com/JZacharie/Pylos/issues/125) | Une réponse d'agent pourrait être servie à un autre locataire |
| Charge de travail (28 pts MCP + 28 pts du plan sécurité) | #112 → #141 | Un seul développeur ne peut pas tenir les deux plans simultanément : voir le calendrier §4 |

### 2.4 Definition of Done (Phase 1)

- [ ] `GET /mcp/tools/list` renvoie les tools autorisés pour la Virtual Key.
- [ ] Un client peut envoyer `chat/completions` avec des MCP tools et Pylos les exécute.
- [ ] Un tool non autorisé est refusé (liste **et** exécution).
- [ ] Tests d'intégration sur mock MCP server (3 transports) + test de la boucle complète.
- [ ] `clippy -D warnings` + `cargo test --workspace` verts ; pas de régression routage/budget/guardrails.

### 2.5 Action immédiate

Créer la story **S-MCP-00** et la branche `feature/s-mcp-00-transport-config` : c'est le seul prérequis bloquant du Sprint A (modèle `McpServer` + migrations SQLite 020/Postgres 022 + DTO API).

---

## 3. Plan d'action — Observabilité pipeline Mnemosyne (#47)

**Constat** : Mnemosyne expose déjà son observabilité de pipeline (`GET /api/pipeline/runs`, `GET /api/pipeline/runs/{id}`, `GET /api/pipeline/stats`, `POST /api/pipeline/retry`). Pylos n'en consomme rien. L'objectif n'est donc pas de réécrire un moteur, mais d'**offrir une surface d'observation et de correction** dans Pylos.

| Phase | Contenu | Points | Sortie |
|---|---|---|---|
| 1 — Design | Contrat d'interface, modèle de données (run, étape, erreur), authentification inter-services, ADR, maquette UI | 3 | ADR + spécification validées |
| 2 — Lecture | Client HTTP `MNEMOSYNE_URL`, proxy admin authentifié, page UI « Pipeline » (liste, détail d'un run, statistiques) | 5 | Un admin voit l'historique et l'étape en échec |
| 3 — Action | Retry d'un run idempotent, garde-fous RBAC, journal d'audit, métriques Prometheus | 5 | Remédiation depuis Pylos, tracée |
| 4 — Tests & doc | Tests d'intégration avec mock Mnemosyne, runbook, documentation | 3 | Couverture + exploitation documentée |

**Critères d'acceptation** : historique et étapes lisibles ; un échec indique l'étape, le message et le fichier ; retry idempotent et audité ; accès réservé aux admins ; dégradation propre si Mnemosyne est indisponible.
**Décisions à prendre** : authentification Pylos ↔ Mnemosyne (token de service, mTLS), cloisonnement multi-tenant des runs, pagination/rétention.

Jalon **#6**, échéance **17/01/2027**.

---

## 4. Calendrier consolidé

| Période | Chantier | Jalon |
|---|---|---|
| 13/10 → 25/10 | Sprint 0 sécurité (issues #112 → #118) | #1 (25/10) |
| 27/10 → 08/11 | **MCP Sprint A** (S-MCP-00 → 03, 14) | #5 |
| 09/11 → 20/11 | **MCP Sprint B** (S-MCP-05, 04, 06, 07, 10, 09) | #5 |
| 21/11 → 29/11 | **MCP Sprint C** (S-MCP-08, 11, 12, 13, 15) | #5 (29/11) |
| 22/11 → 22/11 | Sprint 1 gouvernance & fiabilité (#119 → #126) — **à décaler si le MCP occupe tout le temps** | #2 |
| 24/11 → 20/12 | Sprint 2 architecture & qualité (#127 → #135) | #3 |
| 21/12 → 17/01/2027 | **Mnemosyne pipeline observability** (#47) | #6 (17/01) |
| 05/01 → 14/02/2027 | Sprint 3 fonctionnalités & observabilité (#136 → #141) | #4 (14/02) |

**Alerte de charge** : les sprints 0/1 (28 pts) et la Phase 1 MCP (28 pts) couvrent la même fenêtre d'environ 7 semaines. Avec un seul développeur, il faut choisir : (a) MCP démarre après le Sprint 0 et le Sprint 1 glisse d'environ 3 semaines, ou (b) un second contributeur prend le MCP. Le chemin critique MCP ne dépend d'aucune issue du Sprint 1, il peut donc démarrer dès le 27/10 — c'est la variable d'ajustement.

---

## 5. Hygiène de backlog

1. **Fermer #44, #45, #46** (doublons vérifiés) — recommandation argumentée dans les commentaires d'issue.
2. **Conserver #47 et #62** comme issues canoniques (jalons #6 et #5).
3. **Rapatrier les artefacts BMAD** (`_bmad-output/…`, `_bmad-core/data/epics/…`) dans `docs/` : ils ne sont pas versionnés et le plan MCP y fait référence.
4. **Une PR par story** avec `Closes #NNN` ; la protection de `main` exige les 3 checks (`Run Tests`, `MegaLinter`, `Analyze (rust)`) + branche à jour.

---

*Document généré le 11/10/2026 à partir du commit `e0c6d37`. Suivi : [jalons](https://github.com/JZacharie/Pylos/milestones) · [issues](https://github.com/JZacharie/Pylos/issues).*
