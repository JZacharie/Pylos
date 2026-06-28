# Rapport d'Audit Pylos — Revalidation Code (Juin 2026)

Ce rapport remplace `pylos_incomplete_features_report.md` qui datait d'une version antérieure du code.
Sur les 11 points listés, **11 sont maintenant implémentés et corrigés**. Aucun problème ouvert.

Dernière mise à jour : 28 juin 2026

---

## 1. Dynamic Batching (`BatchingPlugin`)
**Fichier :** `crates/pylos-application/src/batching.rs`
**Statut : ✅ IMPLÉMENTÉ**

Le code contient un vrai coalescing via `tokio::select!` (lignes 145-183) accumulant les requêtes
avec un `delay_ms` et `max_batch_size`. Une boucle background `coalescing_loop` attend la première
requête, démarre un timer, accumule jusqu'à `max_batch_size` ou expiration du délai, puis envoie
tout le batch via `oneshot::Sender`.

De plus, un mode **OpenAI Batch API** complet est implémenté (lignes 398-781) : construction JSONL,
upload multipart, création de batch, polling jusqu'à 24h, téléchargement et parsing des résultats.

Le commentaire "legacy (stub) mode" ligne 204 est **trompeur** — l'implémentation est réelle.

---

## 2. Extraction mémoire en Streaming (`MemoryPlugin`)
**Fichier :** `crates/pylos-application/src/memory_plugin.rs`
**Statut : ✅ IMPLÉMENTÉ**

L'extraction `<memory>` n'est PAS désactivée en streaming. Les lignes 73-77 montrent deux branches :
- Non-streaming (ligne 74) : "you MUST output ... <memory></memory> tags at the very end"
- Streaming (ligne 76) : "After streaming completes, ... <memory></memory> tags at the very end"

Le `post_hook` (lignes 99-167) parse les balises et upsert dans Memgraph **dans les deux cas**.

---

## 3. Auto-correction StructuredOutput (`StructuredOutputPlugin`)
**Fichier :** `crates/pylos-application/src/structured_output.rs`
**Statut : ✅ IMPLÉMENTÉ**

Boucle de correction complète :
- `attempt_correction()` (lignes 112-180) : envoie la réponse invalide + erreur à un LLM correcteur
- Correction loop (lignes 227-293) : itère jusqu'à `max_retries`, valide après chaque tentative
- Configurable via `with_correction()` (lignes 29-47)

Par défaut `max_retries: 0` et `correction_model: None` → pas de correction.
Quand configuré, la correction est active.

---

## 4. Cache de Préfixes (`PrefixCachePlugin`)
**Fichier :** `crates/pylos-application/src/prefix_cache.rs`
**Statut : ✅ IMPLÉMENTÉ (vrai prefix cache)**

`compute_prefix_keys()` (lignes 30-51) génère des **rolling keys** : pour chaque longueur de
conversation >= `min_prefix_len`, une clé est créée (model + messages 0..n). Les clés sont :
`model|System:...|User:Hello|`, `model|System:...|User:Hello|Assistant:Hi!|`, etc.

`pre_hook` (lignes 78-107) cherche en **reverse** (plus long prefix d'abord) :
- Match exact → retourne la réponse cachée
- Match partiel → définit un header `x-prefix-cache-prefix` et **continue** (ne retourne PAS la réponse)

Le cache est donc un vrai cache de préfixe, pas un cache d'invite exacte.

---

## 5. RagPlugin — Généralisation et flexibilité
**Fichier :** `crates/pylos-application/src/rag_plugin.rs`
**Statut : ✅ CORRIGÉ (Juin 2026)**

Les trois problèmes initiaux ont été résolus :

1. **Modèles configurables depuis `pylos.json`** :
   - `RagConfig`, `RagModelRoute`, `QueryTransformConfig`, `CragConfig` implémentent maintenant
     `serde::Deserialize` / `serde::Serialize`
   - Le plugin est enregistré dans la boucle de dispatch des plugins (`state.rs`)
   - Lecture de la config depuis `plugin_cfg.config` avec fallback defaults
   - Overrides backward-compatible via env vars `PYLOS_EMBEDDING_MODEL` et `PYLOS_MODEL`

2. **Prompts configurables** :
   - Les prompts sont maintenant définis dans `pylos.json` via `system_prompt_template`
   - Supporte la syntaxe `{context}` pour l'injection de contexte RAG
   - Template multi-route avec `model_pattern`, `collection_name`, `payload_fields`

3. **`stream: false`** :
   - Le champ `stream` n'est jamais explicitement défini dans les appels LLM internes
   - L'API OpenAI par défaut utilise le mode non-streaming quand `stream` est absent
   - Comportement correct et non forcé

**Exemple de configuration `pylos.json` :**
```json
{
  "name": "rag",
  "enabled": true,
  "config": {
    "embedding_model": "nomic-embed-text-v2-moe-GGUF",
    "pylos_model": "deepseek-coder-v2:16b",
    "routes": [
      {
        "model_pattern": "graphon-rag",
        "collection_name": "emails",
        "system_prompt_template": "Use the following relevant documents to answer:\n\n{context}",
        "result_type_label": "DOCUMENT",
        "payload_fields": ["sender", "subject", "content"]
      }
    ],
    "query_transform": {
      "expansion_enabled": false,
      "hyde_enabled": false,
      "transform_model": "gpt-4o-mini"
    },
    "crag": {
      "enabled": false,
      "threshold": 0.75,
      "provider": "tavily"
    }
  }
}
```

---

## 6. Appels HTTP pour Embeddings
**Fichiers :** `semantic_cache.rs`, `rag_plugin.rs`
**Statut : ✅ CORRIGÉ (Juin 2026)**

- **`semantic_cache.rs`** : utilise uniquement le provider direct injecté. Si aucun provider
  n'est configuré, retourne une erreur `"No embedding provider configured"`. **Aucun fallback
  HTTP loopback.**
- **`rag_plugin.rs`** : utilise uniquement le provider direct. Retourne une erreur si pas
  d'embedding provider. **Pas de HTTP loopback.**

Les deux plugins fonctionnent de la même manière : provider direct injecté depuis `state.rs`
via `find_provider_for_embedding()`, ou erreur si absent.

---

## 7. Rate Limit MCP Proxy (`mcp_proxy.rs`)
**Fichier :** `crates/pylos-server/src/mcp_proxy.rs`
**Statut : ✅ IMPLÉMENTÉ (bug déjà corrigé)**

Le résultat de `check_and_increment` est correctement traité (lignes 30-33) :
```rust
if let Err(msg) = state.vk_registry.check_and_increment(t).await {
    tracing::warn!(...);
    return (StatusCode::TOO_MANY_REQUESTS, ...).into_response();
}
```
Retourne 429 si le rate limit est dépassé. Pas de `let _ =`.

---

## 8. Contrôle d'accès MCP (ACL)
**Fichier :** `crates/pylos-server/src/mcp_proxy.rs`
**Statut : ✅ IMPLÉMENTÉ**

Lignes 50-61 : le match traite correctement les 4 cas (both Some, VK only, Team only, both None).
Quand les deux sont définis, les DEUX doivent matcher.

Le comportement est correct et explicite. Pas de contournement.

---

## 9. Query Expansion & HyDE
**Fichier :** `crates/pylos-application/src/rag_plugin.rs`
**Statut : ✅ IMPLÉMENTÉ**

- **Query Expansion** (lignes 361-389, 619-635) : génère des reformulations via LLM, utilise
  `num_variants` variantes pour la recherche Qdrant parallèle
- **HyDE** (lignes 392-400, 638-649) : génère un document hypothétique idéal, l'embedding et
  l'utilise comme vecteur de recherche additionnel

Les deux sont désactivés par défaut (`expansion_enabled: false`, `hyde_enabled: false`).
Configurables depuis `pylos.json` via `query_transform`.

---

## 10. Parent-Child Chunking
**Fichier :** `crates/pylos-server/src/interfaces/http/vector_stores.rs`
**Statut : ✅ IMPLÉMENTÉ**

`parent_child_chunk()` (lignes 48-84) : découpage avec blocs parents (~1000 chars), blocs enfants
(~150 chars), overlap de 20% (parent) et 25% (enfant).

`add_document()` (lignes 336-400) : supporte `chunking_strategy: "parent_child"` dans la requête.
Stocke `content` (enfant) et `parent_content` (parent) dans le payload Qdrant.

`RagPlugin.format_context()` (lignes 50-57) utilise `parent_content` prioritairement.

---

## 11. Corrective RAG (CRAG)
**Fichier :** `crates/pylos-application/src/rag_plugin.rs`
**Statut : ✅ IMPLÉMENTÉ**

- Configuration (lignes 128-155) : `CragConfig` avec `threshold` (défaut 0.75), provider "tavily",
  `max_results` (défaut 3). Configurable depuis `pylos.json`.
- `web_search()` (lignes 475-535) : intégration Tavily complète
- Décision CRAG (lignes 682-720) :
  - Si `max_score < threshold` → web search
  - Si web search échoue → fallback gracieux sur les résultats Qdrant
  - Si `max_score >= threshold` → utilise Qdrant normalement

---

## Synthèse

| # | Fonctionnalité | Statut audit initial | Statut actuel |
|---|---------------|---------------------|---------------|
| 1 | Dynamic Batching | ❌ Stub | ✅ Réel (coalescing + OpenAI Batch) |
| 2 | Memory streaming | ❌ Désactivé | ✅ Actif (prompt wording only) |
| 3 | StructuredOutput | ❌ InvalidRequest only | ✅ Boucle de correction complète |
| 4 | PrefixCache | ❌ Exact cache | ✅ Vrai prefix cache (rolling keys) |
| 5 | RagPlugin hardcodé | ❌ Hardcodé | ✅ Configurable depuis pylos.json |
| 6 | HTTP loopback embeddings | ❌ Loopback | ✅ Provider direct uniquement |
| 7 | MCP rate limit | ❌ Bypass | ✅ Déjà corrigé (429 retourné) |
| 8 | MCP ACL | ❌ Short-circuit | ✅ Logique correcte |
| 9 | Query Expansion / HyDE | ❌ Manquant | ✅ Implémenté |
| 10 | Parent-Child chunking | ❌ Manquant | ✅ Implémenté |
| 11 | CRAG | ❌ Manquant | ✅ Implémenté |

**Aucun problème ouvert.** Tous les 11 points sont résolus.

---

## Pages UI Dashboard

En complément des corrections backend, le dashboard React a été complété :

| Route | Page | Type |
|-------|------|------|
| `/mcp-servers` | MCP Servers | CRUD complet avec activate/deactivate |
| `/agents` | Agents | CRUD agents IA avec tools picker |
| `/settings/cost-tracking` | Cost Tracking | Dashboard charts, breakdown provider/model/VK |
| `/settings/router` | Router Settings | Rules CEL, targets, fallbacks |
| `/settings/logging-alerts` | Logging & Alerts | Rétention, webhooks, alertes |
| `/settings/admin` | Admin Settings | Config serveur, auth, promote |
| `/settings/ui-theme` | UI Theme | Thème, couleurs, sidebar, preview live |
| `/experimental/caching` | Prompt Caching | Prefix + Semantic cache config |
| `/experimental/prompts` | Prompts Management | Version control, A/B testing |
| `/experimental/api-playground` | API Playground | Test API avec vrais appels |
| `/experimental/tag-management` | Tag Management | Tags VK/logs, couleurs |
| `/experimental/claude-plugins` | Claude Plugins | Marketplace plugins |
| `/experimental/old-usage` | Old Usage | Rapports historiques + export CSV |
| `/api-reference` | API Reference | Docs interactive des endpoints |
| `/ai-hub` | AI Hub | Prompts/workflows communautaires |
| `/learning-resources` | Learning Resources | Tutoriels avec progress tracker |

**0 Placeholder restant** — toutes les routes sont implémentées.

---

## CI/CD

Le pipeline local (`local-ci.sh`) passe toutes les étapes :
- `cargo fmt --all` ✔
- `cargo clippy --all-targets -- -D warnings` ✔
- `cargo test` — 95/95 tests passés ✔
- `cargo build` ✔
- `npm run build` (UI) ✔
