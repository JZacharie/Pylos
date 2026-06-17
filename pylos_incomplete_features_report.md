# Rapport d'Audit Pylos : Fonctionnalités Incomplètes et Issues Proposées

Ce rapport récapitule les fonctionnalités incomplètes, les comportements de type "stubs" (bouchons), les inefficacités et les failles de sécurité identifiés lors de la revue de code de la passerelle IA **Pylos**. Chaque point est accompagné d'une proposition d'issue technique pour orienter le développement.

---

## Sommaire

1. [Dynamic Batching (`BatchingPlugin`)](#1-dynamic-batching-batchingplugin)
2. [Extraction de mémoire en mode Streaming (`MemoryPlugin`)](#2-extraction-de-mémoire-en-mode-streaming-memoryplugin)
3. [Boucle d'auto-correction de format (`StructuredOutputPlugin`)](#3-boucle-dauto-correction-de-format-structuredoutputplugin)
4. [Gestion du Cache de Préfixes (`PrefixCachePlugin`)](#4-gestion-du-cache-de-préfixes-prefixcacheplugin)
5. [Généralisation et flexibilité (`RagPlugin`)](#5-généralisation-et-flexibilité-ragplugin)
6. [Appels HTTP auto-référencés pour les Embeddings (`SemanticCache` & `RagPlugin`)](#6-appels-http-auto-référencés-pour-les-embeddings-semanticcache--ragplugin)
7. [Bypass du Rate Limit MCP dans le Proxy (`mcp_proxy_handler`)](#7-bypass-du-rate-limit-mcp-dans-le-proxy-mcp_proxy_handler)
8. [Contrôle d'accès défaillant (ACL) sur les serveurs MCP (`mcp_proxy_handler`)](#8-contrôle-daccès-défaillant-acl-sur-les-serveurs-mcp-mcp_proxy_handler)

---

## 1. Dynamic Batching (`BatchingPlugin`)

*   **Fichier concerné :** [batching.rs](file:///home/joseph/git/Pylos/crates/pylos-application/src/batching.rs)
*   **Problème :** Le plugin applique un délai configuré pour accumuler les requêtes concourantes, mais il ne réalise aucun regroupement réel au niveau du transport vers le fournisseur de modèle (comme l'utilisation de l'API Batch d'OpenAI ou le regroupement des requêtes). Il affiche un avertissement expliquant que la fonctionnalité n'est pas encore complètement implémentée.

### Issue Proposée

```markdown
Title: [Feature] Implement real Request Coalescing / OpenAI Batch API in BatchingPlugin

Description:
Currently, `BatchingPlugin` only introduces a delay using `tokio::time::sleep` and a mutex to simulate request pooling, but it sends requests individually to downstream providers.

Tasks:
- Implement a request queue (coalescing window) that groups concurrent requests for the same model.
- Add integration with the OpenAI Batch API (`/v1/batch`) for asynchronous, cheaper off-peak processing.
- Provide configuration options to select between real-time batching (coalescing) and asynchronous batching.
```

---

## 2. Extraction de mémoire en mode Streaming (`MemoryPlugin`)

*   **Fichier concerné :** [memory_plugin.rs](file:///home/joseph/git/Pylos/crates/pylos-application/src/memory_plugin.rs)
*   **Problème :** L'extraction des relations `<memory>...</memory>` à injecter dans Memgraph est explicitement désactivée lorsque la requête utilise le streaming (`chat_req.stream = true`). Par conséquent, l'assistant n'apprend aucune information sur l'utilisateur lorsque ce dernier utilise le mode streaming, ce qui brise l'expérience utilisateur globale.

### Issue Proposée

```markdown
Title: [Feature] Support Memory extraction in Streaming mode for MemoryPlugin

Description:
The `MemoryPlugin` only injects memory extraction instructions and parses the `<memory>` tags from the assistant response when `stream` is false. We need to support this for streaming completions as well.

Tasks:
- Implement a streaming buffer hook in `post_hook` (or via stream interception) that collects the streamed tokens.
- Parse the final compiled response to extract `<memory>` tags and upsert them to Memgraph (neo4rs) after the stream completes.
```

---

## 3. Boucle d'auto-correction de format (`StructuredOutputPlugin`)

*   **Fichier concerné :** [structured_output.rs](file:///home/joseph/git/Pylos/crates/pylos-application/src/structured_output.rs)
*   **Problème :** La documentation (par exemple `notebooklm.md`) indique que le plugin StructuredOutput fournit une "validation et correction automatique des réponses JSON". En réalité, le code se contente de rejeter la requête avec une erreur `PylosError::InvalidRequest` si le JSON est mal formé ou invalide par rapport au schéma. Il n'y a aucune boucle de correction automatique ou de second appel à l'LLM pour corriger l'erreur de formatage.

### Issue Proposée

```markdown
Title: [Feature] Implement automatic correction loop for StructuredOutputPlugin

Description:
If the response fails JSON or JSON Schema validation, the plugin currently returns an immediate error to the client. We want to implement an automatic correction loop using a secondary fast LLM call (or self-correction prompt).

Tasks:
- If validation fails, intercept the response.
- Send a correction prompt to a fast model (e.g., GPT-4o-mini / Gemini Flash) including the invalid JSON and the validation error message.
- Return the corrected JSON to the client. Ensure a maximum retry limit (e.g., 1 or 2 retries) to prevent infinite loops.
```

---

## 4. Gestion du Cache de Préfixes (`PrefixCachePlugin`)

*   **Fichier concerné :** [prefix_cache.rs](file:///home/joseph/git/Pylos/crates/pylos-application/src/prefix_cache.rs)
*   **Problème :** Bien qu'il s'appelle `prefix_cache`, la clé est générée à partir de la concaténation de la totalité des messages du chat. Ce n'est donc pas un cache de préfixe (comme le propose le KV cache partagé des LLM), mais un simple cache d'invite exacte (exact prompt cache).

### Issue Proposée

```markdown
Title: [Refactor] Implement true Prefix/Sub-prompt matching in PrefixCachePlugin

Description:
Currently, `PrefixCachePlugin` hashes the entire message history, meaning any change in the last message results in a cache miss. A true prefix cache should identify common prefixes (like system instructions or early conversation turns).

Tasks:
- Modify the key generation to check for common prefixes (e.g., matching the first N messages, or system prompts).
- Allow returning partial cached states or caching specific sub-trees of conversation history.
- Rename the plugin or adjust the documentation if it is meant to remain a simple exact prompt cache.
```

---

## 5. Généralisation et flexibilité (`RagPlugin`)

*   **Fichier concerné :** [rag_plugin.rs](file:///home/joseph/git/Pylos/crates/pylos-application/src/rag_plugin.rs)
*   **Problème :** Le plugin RAG contient de nombreuses chaînes de caractères codées en dur : les modèles ciblés (`graphon-rag`, `mnemosyne-search`), les prompts de système en français, et il force la complétion sous-jacente à être non-streaming (`"stream": false`), ignorant l'option demandée par l'utilisateur.

### Issue Proposée

```markdown
Title: [Enhancement] Generalize RagPlugin config (Dynamic prompts, Model configurations, and Streaming support)

Description:
The RAG plugin is currently coupled to specific French prompts, hardcoded model IDs, and completely disables streaming responses.

Tasks:
- Move targeted models, Qdrant collections, and prompt templates to the configuration file (`pylos.json`).
- Add multi-language support or templating for RAG prompts.
- Implement streaming support by reconstructing the SSE stream with augmented context instead of forcing `stream: false`.
```

---

## 6. Appels HTTP auto-référencés pour les Embeddings (`SemanticCache` & `RagPlugin`)

*   **Fichiers concernés :** [semantic_cache.rs](file:///home/joseph/git/Pylos/crates/pylos-application/src/semantic_cache.rs#L58-L117) et [rag_plugin.rs](file:///home/joseph/git/Pylos/crates/pylos-application/src/rag_plugin.rs#L98-L150)
*   **Problème :** Pour obtenir l'embedding d'une requête utilisateur, ces deux plugins envoient une nouvelle requête HTTP sur le réseau à Pylos lui-même via `pylos_base_url/v1/embeddings`. Cela engendre une surconsommation réseau, de la sérialisation/désérialisation inutile, et présente un risque de blocage (deadlock) si le pool de connexions HTTP local vient à saturation.

### Issue Proposée

```markdown
Title: [Performance] Call InferenceOrchestrator directly for embeddings in Plugins

Description:
`SemanticCachePlugin` and `RagPlugin` fetch embeddings by making a full loopback HTTP request to `pylos_base_url/v1/embeddings`. We should bypass the HTTP stack and call the rust orchestration layer directly.

Tasks:
- Inject the `InferenceOrchestrator` (or the internal embedding service) into the plugins during initialization.
- Replace HTTP calls to `/v1/embeddings` with direct Rust function calls (`orchestrator.embed(...)`).
```

---

## 7. Bypass du Rate Limit MCP dans le Proxy (`mcp_proxy_handler`)

*   **Fichier concerné :** [mcp_proxy.rs](file:///home/joseph/git/Pylos/crates/pylos-server/src/mcp_proxy.rs#L28-L38)
*   **Problème :** Dans le handler `mcp_proxy_handler`, la ligne `let _ = state.vk_registry.check_and_increment(t).await;` appelle bien la vérification du rate limit sur la Virtual Key, mais elle ignore complètement le résultat (la valeur de retour `Result` est ignorée via `let _ = ...`). Si la clé virtuelle dépasse ses quotas RPM (requêtes par minute), l'accès aux serveurs MCP restera quand même autorisé.

### Issue Proposée

```markdown
Title: [Bug] MCP Proxy bypasses Virtual Key Rate Limiting

Description:
In `mcp_proxy_handler`, the check to the virtual key registry rate limit is called, but the result is discarded with `let _ = ...`. Consequently, requests to MCP tools are not rate-limited and will succeed even if the key is rate-limited.

Tasks:
- Capture the result of `state.vk_registry.check_and_increment(t).await`.
- If it returns an `Err(reason)`, return a `429 Too Many Requests` or `401 Unauthorized` response to the client, matching the behavior of `virtual_key_middleware.rs`.
```

---

## 8. Contrôle d'accès défaillant (ACL) sur les serveurs MCP (`mcp_proxy_handler`)

*   **Fichier concerné :** [mcp_proxy.rs](file:///home/joseph/git/Pylos/crates/pylos-server/src/mcp_proxy.rs#L50-L55)
*   **Problème :** Lors du filtrage du serveur MCP actif correspondant :
    ```rust
    && (match (&s.virtual_key_id, &s.team_id) {
        (Some(vk_id), _) => virtual_key_id.as_ref() == Some(vk_id),
        (_, Some(t_id)) => team_id.as_ref() == Some(t_id),
        (None, None) => false,
    })
    ```
    Si un serveur MCP possède à la fois un `virtual_key_id` et un `team_id`, le premier bras du match `(Some(vk_id), _)` est sélectionné, ignorant totalement la vérification du `team_id`. De plus, si les deux sont définis mais que seul l'un d'eux correspond, la requête passe alors qu'une politique stricte exigerait la validation des deux critères (ou au moins une logique explicite AND/OR).

### Issue Proposée

```markdown
Title: [Bug] Incomplete Access Control checks for MCP Servers in Proxy

Description:
The pattern matching for checking permissions on MCP servers (`virtual_key_id` and `team_id`) currently short-circuits. If both are set, it only checks the virtual key, ignoring the team verification.

Tasks:
- Redesign the ACL verification logic to handle cases where both `virtual_key_id` and `team_id` are set (either applying AND or OR based on the security policy).
- Ensure clear separation of checks, returning 403 Forbidden if the verification fails.
```
