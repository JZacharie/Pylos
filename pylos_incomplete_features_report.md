# Rapport d'Audit Pylos — Revalidation Code (Juin 2026)

Ce rapport remplace `pylos_incomplete_features_report.md` qui datait d'une version antérieure du code.
Sur les 11 points listés, 7 sont déjà implémentés et 4 nécessitent encore du travail.

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
**Statut : ❌ CONFIRMÉ (hardcodé)**

Trois problèmes confirmés :
1. **Modèles par défaut hardcodés** (lignes 177-180) :
   - Embedding : `"nomic-embed-text-v2-moe-GGUF"` (surrideable via `PYLOS_EMBEDDING_MODEL`)
   - LLM : `"deepseek-coder-v2:16b"` (surrideable via `PYLOS_MODEL`)
2. **Prompts en français** (lignes 186, 194, 202) :
   `"Utilise les documents pertinents suivants pour répondre à l'utilisateur de manière précise, concise:"`
   — pas de template multi-langue, pas de configuration externe
3. **`stream: false` forcé** (ligne 314) : les appels LLM internes pour query transformation sont
   systématiquement non-streaming. Non configurable.

**Correction proposée :**
- Déplacer les modèles cibles, les templates de prompt et le comportement streaming dans `pylos.json`
- Ajouter un support multi-langue ou templating
- Supprimer le `stream: false` forcé ou le rendre optionnel

---

## 6. Appels HTTP pour Embeddings
**Fichiers :** `semantic_cache.rs`, `rag_plugin.rs`
**Statut : ⚠️ PARTIELLEMENT CORRIGÉ**

- **`semantic_cache.rs`** (lignes 66-150) : essaie d'abord le provider direct, **fallback** HTTP
  loopback si pas de provider configuré. Le fallback existe toujours.
- **`rag_plugin.rs`** (lignes 264-291) : utilise uniquement le provider direct. Retourne une erreur
  si pas d'embedding provider. **Pas de HTTP loopback.**

Le vrai problème est dans `semantic_cache.rs` où le fallback HTTP existe encore.

**Correction proposée :**
- Supprimer le fallback HTTP loopback dans `semantic_cache.rs`
- Injection directe de l'`InferenceOrchestrator` dans les plugins

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
**Statut : ⚠️ LOGIQUE OK MAIS PAS DE FALLBACK**

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
  `max_results` (défaut 3)
- `web_search()` (lignes 475-535) : intégration Tavily complète
- Décision CRAG (lignes 682-720) :
  - Si `max_score < threshold` → web search
  - Si web search échoue → fallback gracieux sur les résultats Qdrant
  - Si `max_score >= threshold` → utilise Qdrant normalement

---

## Synthèse

| # | Fonctionnalité | Statut dans l'audit | Réalité |
|---|---------------|-------------------|---------|
| 1 | Dynamic Batching | ❌ Stub | ✅ Réel (coalescing + OpenAI Batch) |
| 2 | Memory streaming | ❌ Désactivé | ✅ Actif (prompt wording only) |
| 3 | StructuredOutput | ❌ InvalidRequest only | ✅ Boucle de correction complète |
| 4 | PrefixCache | ❌ Exact cache | ✅ Vrai prefix cache (rolling keys) |
| 5 | RagPlugin hardcodé | ❌ Hardcodé | ❌ Hardcodé (à corriger) |
| 6 | HTTP loopback embeddings | ❌ Loopback | ⚠️ semantic_cache: oui, rag_plugin: non |
| 7 | MCP rate limit | ❌ Bypass | ✅ Déjà corrigé (429 retourné) |
| 8 | MCP ACL | ❌ Short-circuit | ✅ Logique correcte |
| 9 | Query Expansion / HyDE | ❌ Manquant | ✅ Implémenté |
| 10 | Parent-Child chunking | ❌ Manquant | ✅ Implémenté |
| 11 | CRAG | ❌ Manquant | ✅ Implémenté |

**Il reste 3 vrais problèmes à corriger :**
1. **RagPlugin** : modèles hardcodés, prompts français, `stream: false` forcé
2. **SemanticCache** : fallback HTTP loopback pour les embeddings
3. **Aucun bug dans le code review précédent** (commit `b2f507f`) : les 3 corrections
   (empty allowed_models guard, matches_pattern globs, rate limiting VK management) sont valides
