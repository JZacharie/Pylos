# Pylos — AI Gateway & MCP Proxy

## Présentation générale

Pylos est un **AI Gateway** haute performance, réécrit en Rust, qui sert de point d'accès unifié à **20+ fournisseurs de modèles de langage (LLM)**. Il est compatible avec les SDK OpenAI : il suffit de pointer son `base_url` vers Pylos pour bénéficier de l'ensemble des fonctionnalités sans changer une ligne de code.

---

## 1. Gestion des Tokens et Optimisation des Coûts LLM

Pylos réduit drastiquement la consommation de tokens et le coût des appels LLM via une combinaison de techniques complémentaires. L'économie réalisée peut atteindre **60-80%** sur la facture LLM selon les cas d'usage.

### 1.1 Cache Sémantique (SemanticCachePlugin)

Le cache sémantique stocke les réponses LLM dans **Qdrant** (base vectorielle) et les réutilise pour des requêtes similaires.

**Fonctionnement :**
1. La requête utilisateur est convertie en **vecteur d'embedding**
2. Recherche par similarité cosine dans Qdrant (seuil configurable, défaut: 0.90)
3. Si une réponse similaire existe → retour immédiate (**0 requête LLM**)
4. Si absence → appel LLM normal, puis la réponse est stockée dans Qdrant pour les futures requêtes

**Gain :** Les questions fréquentes ou reformulations ne consomment aucun token. Particulièrement efficace pour le support client, les FAQs, les dashboards avec questions types.

**Configuration :** Seuil de similarité, TTL (time-to-live), modèle d'embedding, collection Qdrant.

### 1.2 Cache de Préfixes (PrefixCachePlugin)

Cache en mémoire (via **Moka**) qui identifie les séquences de messages identiques en début de conversation.

**Fonctionnement :**
1. Génération de clés pour chaque **préfixe de messages** (messages 1..N)
2. Recherche du plus long préfixe correspondant dans le cache
3. Si exact match → réponse instantanée (**0 token LLM**)
4. Si préfixe partiel → le cache est marqué comme chaud pour le provider (KV cache hit)

**Gain :** Les conversations partageant un même historique (ex: même system prompt + premières interactions) évitent de reprocesser les tokens initiaux. Le KV cache des LLM est réutilisé, réduisant la latence TTFT de 40-60%.

### 1.3 Cache Aligner (CacheAlignerPlugin)

Analyse et **nettoie le contenu volatil** des messages avant leur envoi aux LLM pour maximiser l'efficacité du cache KV des providers.

**Détection et anonymisation :**
- UUIDs → remplacés par `{UUID}`
- Dates ISO 8601 → remplacées par `{DATE}`
- Tokens JWT → remplacés par `{JWT}`
- Hex hashs longs → remplacés par `{HASH}`
- UUIDs en snake_case (ex: `user_id_550e8400...`) → préservés mais normalisés

**Gain :** Sans ce plugin, chaque appel LLM avec des UUIDs/dates différentes génère un **cache miss** complet du KV cache provider (DeepSeek, Anthropic, OpenAI). Avec l'alignement, le system prompt et les messages stables restent dans le cache, réduisant le temps de prétraitement de **30-50%** et le coût des tokens de cache miss.

### 1.4 Batching Intelligent (BatchingPlugin)

Deux modes de regroupement des requêtes concurrentes :

**Mode Coalescing (temps réel) :**
- Accumule les requêtes vers le même modèle pendant une fenêtre de temps configurable (ex: 50ms)
- Les libère simultanément pour permettre au provider de les batch-er au niveau transport
- Idéal pour les providers qui offrent des réductions par lot

**Mode Async Batch (OpenAI Batch API) :**
- Accumule les requêtes et les soumet via l'API `/v1/batches` d'OpenAI
- Prix réduit de **50%** par rapport à l'API temps réel
- Résultats récupérés par polling, stockés dans un store partagé
- Parfait pour les workloads non urgents : analyse de logs, génération de rapports, traitement par lot

**Gain :** 50% d'économie sur les requêtes décalables via le mode async batch.

### 1.5 Compression Caveman (CacheAlignerPlugin)

Algorithme de compression de contenu qui réduit la taille des messages sans perte sémantique :
- Codes barrés : lignes de code indentées > 3 niveaux → tronquées avec compteur
- Contenu textuel long → taux de compression configurable
- Garde-fou automatique : si le modèle répond avec des `{...}` non résolus, la compression est réduite automatiquement

**Gain :** Réduction de **20-40%** du nombre de tokens par requête.

### 1.6 Route-Based Model Selection (Model Mapping)

Pylos traduit automatiquement les noms de modèles entre fournisseurs lors des fallbacks, permettant d'utiliser le **modèle le moins cher disponible** pour chaque requête.

**Exemple :** `gemini-2.5-flash` → `deepseek-v4-flash` (si DeepSeek est moins cher)
**Règle :** Les modèles "pro" (gpt-4, claude-opus, gemini-pro) sont mappés vers des équivalents pros; les modèles "fast" (flash, mini, haiku) vers des équivalents économiques.

**Gain :** Économie de **30-70%** en routant vers les modèles les plus compétitifs pour chaque niveau de qualité.

### 1.7 Budget Enforcement (BudgetPlugin)

Limites budgétaires en USD par clé virtuelle avec périodes de reset configurables. Agit comme un **garde-fou financier** pour éviter les dépassements.

---

## 2. Architecture des Plugins (Pre/Post Hooks)

| Plugin | Fonction | Type |
|---|---|---|
| **CacheAligner** | Nettoie le contenu volatil pour maximiser le cache KV | Pre-hook |
| **SemanticCache** | Cache sémantique vectoriel via Qdrant | Pre-hook + Post-hook |
| **PrefixCache** | Cache de préfixes de conversations en mémoire | Pre-hook + Post-hook |
| **RAG** | Retrieval-Augmented Generation avec contexte vectoriel | Pre-hook |
| **Memory** | Mémoire cross-agent via graphe de connaissances | Pre-hook + Post-hook |
| **StructuredOutput** | Validation et correction automatique des réponses JSON | Post-hook |
| **Guardrails** | Filtrage PII, mots-clés, injections prompt | Pre-hook |
| **Budget** | Enforcement des limites budgétaires USD | Pre-hook |
| **RateLimit** | Enforcement des limites RPM/TPM | Pre-hook |
| **Batching** | Regroupement de requêtes (coalescing + async batch) | Pre-hook |
| **PromptRegistry** | Registre de templates de system prompts | Pre-hook |
| **OpenTelemetry** | Attribution de span attributes pour tracing | Pre-hook |

**Ordre d'exécution :**
```
Pre-hooks (ordre d'enregistrement) → InferenceOrchestrator → Post-hooks (ordre inverse)
```

Le pipeline complet permet à chaque plugin de **modifier la requête** avant l'appel LLM, et d'**analyser/réécrire la réponse** après.

---

## 3. RAG (Retrieval-Augmented Generation)

### 3.1 Concept Général

Le RAG permet d'enrichir les prompts LLM avec des connaissances externes issues d'une base vectorielle. Au lieu de faire appel à la seule mémoire paramétrique du modèle, Pylos injecte du **contexte récupéré dynamiquement** depuis Qdrant, garantissant des réponses précises, actualisées et traçables.

### 3.2 Routes et Configuration

Pylos supporte des **routes RAG configurables** via `RagConfig` :

```json
{
  "routes": [
    {
      "model_pattern": "mon-modele-rag-*",
      "collection_name": "ma_collection",
      "system_prompt_template": "Contexte:\n{context}\n\nRéponds à l'utilisateur:",
      "payload_fields": ["title", "content"]
    }
  ]
}
```

Chaque route associe un **pattern de modèle LLM** (avec support wildcard `*`) à :
- Une **collection Qdrant** cible
- Un **template de prompt** avec placeholder `{context}`
- Les **champs de payload** à extraire des documents

### 3.3 Pipeline RAG Complet

```
Requête utilisateur
       │
       ▼
┌─────────────────────┐
│ Transformation      │ ← Query Expansion (génération de variantes)
│ de la requête       │ ← HyDE (document hypothétique)
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ Embedding           │ ← Conversion texte → vecteur
│ (via provider direct)│    (modèle configurable)
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ Recherche Qdrant    │ ← Similarité cosine, top-K
│ (parallélisée)      │    Multi-requêtes si expansion
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ CRAG (Corrective)   │ ← Si score < seuil → recherche web
│                     │    Provider: Tavily, SearxNG, etc.
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ Injection contexte  │ ← Messages système augmentés
│ + appel LLM         │    Pipeline normal
└─────────────────────┘
```

### 3.4 Query Expansion

Avant la recherche vectorielle, Pylos peut **générer des variantes** de la question utilisateur via un LLM rapide (GPT-4o-mini, Gemini Flash).

**Exemple :**
- Requête originale : "Quel est le prix du forfait entreprise ?"
- Variante 1 : "Combien coûte l'abonnement pro ?"
- Variante 2 : "Tarifs formule société"
- Variante 3 : "Pricing plan business"

Chaque variante est embeddée et recherchée en parallèle dans Qdrant via `futures::join_all`. Les résultats sont fusionnés avec déduplication.

**Gain :** Amélioration du rappel de recherche de **15-30%** pour les questions ambiguës ou mal formulées.

### 3.5 HyDE (Hypothetical Document Embeddings)

Technique qui consiste à **générer un document idéal hypothétique** qui répondrait parfaitement à la question, puis à utiliser cet embedding pour la recherche.

**Principe :** Au lieu de chercher avec l'embedding de la question, on cherche avec l'embedding du **document de réponse idéal**. Cela fonctionne mieux car le document généré ressemble structurellement aux vrais documents dans Qdrant.

**Gain :** Amélioration de la précision de **10-20%** par rapport à une recherche directe.

### 3.6 Parent-Child Chunking

Stratégie de découpage des documents en deux niveaux :

```
Document complet
       │
       ▼
┌─────────────────────┐
│ Parents blocks      │ ← ~1000 tokens, chevauchement 20%
│ (contexte riche)    │
├─────────────────────┤
│ Enfants blocks      │ ← ~150 tokens, chevauchement 25%
│ (embedding précis)  │
└─────────────────────┘
```

**Fonctionnement :**
- Les **enfants** sont embeddés et indexés dans Qdrant (recherche précise)
- Le **parent** complet est stocké dans le payload de l'enfant
- Lors de la recherche, c'est le contenu du **parent** qui est injecté dans le prompt LLM

**Avantage :** La recherche trouve des passages très précis (petits chunks enfants) mais le LLM reçoit un **contexte riche et complet** (gros chunk parent). Évite les réponses hors contexte dues à des fragments trop petits.

**Utilisation via API :**
```http
POST /api/vector-stores/collections/{name}/points
{
  "text": "Long document...",
  "embedding_model": "nomic-embed-text",
  "chunking_strategy": "parent_child"
}
```

### 3.7 CRAG (Corrective RAG)

Mécanisme de **détection de confiance** qui déclenche une recherche web de secours si les résultats vectoriels sont de mauvaise qualité.

**Algorithme :**
1. Recherche Qdrant → score de similarité max = 0.62
2. Seuil configuré = 0.75
3. **CRAG déclenché** → recherche web via Tavily
4. Résultats web injectés comme contexte à la place des résultats Qdrant

**Configuration :**
```json
{
  "crag": {
    "enabled": true,
    "threshold": 0.75,
    "provider": "tavily",
    "api_key": "env.TAVILY_API_KEY",
    "max_results": 3
  }
}
```

**Gain :** Élimine les **hallucinations par manque de contexte**. Garantit que le LLM reçoit toujours des informations pertinentes, même quand la base vectorielle est lacunaire.

### 3.8 Endpoints Vector Stores

Interface REST complète pour la gestion des collections vectorielles :

| Endpoint | Méthode | Description |
|---|---|---|
| `/api/vector-stores/collections` | GET | Liste toutes les collections Qdrant |
| `/api/vector-stores/collections` | POST | Crée une collection (nom, taille vecteur, distance) |
| `/api/vector-stores/collections/:name` | DELETE | Supprime une collection |
| `/api/vector-stores/collections/:name/points` | POST | Ajoute un document (avec chunking optionnel) |
| `/api/vector-stores/collections/:name/search` | POST | Recherche par similarité sémantique |

---

## 4. Mémoire Cross-Agent (MemoryPlugin)

### 4.1 Concept

Pylos maintient une **mémoire persistante et structurée** des interactions utilisateurs sous forme de **graphe de connaissances** (Knowledge Graph) stocké dans **Memgraph** (base graphe compatible neo4j).

Contrairement à une simple historique de chat, le graphe capture les **relations sémantiques** entre entités :
```
(User) --PREFERS--> (Rust)
(User) --WORKS_ON--> (Project X)
(Project X) --USES--> (PostgreSQL)
```

### 4.2 Fonctionnement

**Phase d'injection (pre-hook) :**
1. Requête Memgraph pour récupérer les relations connues de l'utilisateur (clé virtuelle)
2. Injection des `<memory>` tags comme instruction système au LLM
3. Le LLM est invité à produire de nouvelles relations en fin de réponse

**Phase d'extraction (post-hook) :**
1. Analyse de la réponse du LLM pour détecter les balises `<memory>EntityA|RELATION|EntityB</memory>`
2. Parsing du triplet (entité1, relation, entité2)
3. Upsert dans Memgraph via Cypher `MERGE`
4. Nettoyage de la réponse (les balises sont retirées du texte visible)

**Exemple de cycle mémoire :**
```
User: "Je travaille sur un projet en Rust"
Assistant: "Génial ! Rust est excellent pour les systèmes." <memory>User|WORKS_ON|Rust Project</memory>
                                                  ▲ extraction et sauvegarde dans Memgraph
                                                  ▼ requête suivante
User: "Quel langage est-ce que je préfère ?"
Assistant: "D'après notre conversation précédente, vous travaillez sur un projet en Rust."
```

### 4.3 Support du Streaming

L'extraction mémoire fonctionne **même en mode streaming** :
- L'instruction système est injectée avec une formulation adaptée au streaming
- Après la fin du stream, un wrapper collecte tous les chunks et reconstitue la réponse complète
- Les plugins post-hook (dont MemoryPlugin) sont exécutés sur la réponse reconstituée
- Les balises `<memory>` sont extraites et persistées dans Memgraph

### 4.4 Architecture

```
                  ┌──────────────────┐
                  │    Memgraph DB    │
                  │  (graphe neo4j)   │
                  └────────┬─────────┘
                           │
          ┌────────────────┴────────────────┐
          │          MemoryPlugin            │
          │  pre-hook : injection contexte   │
          │  post-hook : extraction tags     │
          └────────────────┬────────────────┘
                           │
          ┌────────────────┴────────────────┐
          │         InferenceOrchestrator    │
          │     Appel LLM (stream ou non)    │
          └─────────────────────────────────┘
```

### 4.5 Utilisation

Activation dans `pylos.json` :
```json
{
  "plugins": [
    {
      "name": "memory",
      "enabled": true
    }
  ]
}
```

Variable d'environnement : `MEMGRAPH_URL=127.0.0.1:7687`

---

## 5. Structured Output

Pylos valide et **corrige automatiquement** les réponses JSON produites par les LLM.

### 5.1 Modes Supportés

- `json_object` : valide que la réponse est un JSON valide
- `json_schema` : valide contre un JSON Schema fourni dans la requête

### 5.2 Boucle de Correction Automatique

Si la validation échoue :
1. Pylos appelle un **modèle de correction** rapide (ex: GPT-4o-mini)
2. Il envoie le JSON invalide + le message d'erreur de validation
3. Le modèle produit une version corrigée
4. Nouvelle validation
5. Jusqu'à `max_retries` tentatives (défaut: 1)

**Sans correction :** L'application cliente doit gérer l'erreur et relancer
**Avec correction :** Le client reçoit toujours un JSON valide

---

## 6. Compatibilité OpenAI

### Endpoints supportés
- `POST /v1/chat/completions` — Chat completions (streaming et non-streaming)
- `POST /v1/completions` — Completions texte legacy (conversion automatique vers chat)
- `POST /v1/embeddings` — Embeddings
- `POST /v1/images/generations` — Génération d'images
- `GET /v1/models` — Listing des modèles disponibles

### Compatibilité SDK
- **Drop-in replacement** pour les SDK OpenAI (Python, Node.js, etc.)
- Configuration minimale : changer `base_url` et utiliser une clé virtuelle Pylos (`sk-pylos-*`)

---

## 7. Gestion des fournisseurs d'IA

### 7 adaptateurs natifs
| Fournisseur | Protocole |
|---|---|
| **OpenAI** & compatibles (Groq, Ollama, OpenRouter, vLLM, Mistral, Cerebras, Perplexity, Fireworks, xAI, Nebius, DeepSeek, Lemonade, Vertex) | API OpenAI |
| **Anthropic** | API Claude |
| **AWS Bedrock** | Converse / ConverseStream API |
| **Azure OpenAI** | Azure OpenAI Service |
| **Google Gemini** | Gemini API |
| **Cohere** | Cohere API v2 |
| **DeepSeek** | API OpenAI-compatible |

### Auto-détection des fournisseurs
- Détection automatique du fournisseur à partir du nom du modèle
- Exemples : `gpt-*` → OpenAI, `claude-*` → Anthropic, `gemini-*` → Gemini

### Gestion des clés API
- **Multi-clés pondérées** : plusieurs clés API par fournisseur avec poids configurables
- **Load balancing** : algorithme A-Res (weighted random selection)
- **Circuit breaker** : après 5 échecs consécutifs, un fournisseur est désactivé 30 secondes
- **Retry avec backoff exponentiel** et jitter

### Gestion des fournisseurs en runtime
- CRUD complet des fournisseurs via l'API de management ou l'interface web
- Configuration réseau (timeouts, endpoints personnalisés)
- Statut de connectivité visible dans l'interface

---

## 8. Routage intelligent

### Routage par modèle
- Association automatique modèle → fournisseur
- Support des **règles CEL (Common Expression Language)** pour le routage avancé
- Cibles pondérées pour le routage A/B

### Fallback multi-fournisseur
- Si un fournisseur échoue, bascule automatique vers le suivant
- Ordonnancement : fournisseurs supportant le modèle d'abord, puis les autres
- **Model mapping** : traduction des noms de modèles entre fournisseurs (ex: `gemini-2.5-flash` → `deepseek-v4-flash`)

### Streaming
- SSE (Server-Sent Events) avec métriques token-level
- **Time-to-First-Token (TTFT)** et **tokens-per-seconde** trackés
- Support des **tool calls en streaming**
- Support du **reasoning content** (DeepSeek R1, OpenAI o1/o3)

---

## 9. Sécurité et gouvernance

### Clés API virtuelles
- Format `sk-pylos-*`
- **ACL par fournisseur et par modèle** : contrôle granulaire de ce que chaque clé peut utiliser
- Poids (weight) par fournisseur pour la répartition
- Clés avec **date d'expiration**
- Activation/désactivation sans suppression
- Registry in-memory pour lookup ultra-rapide

### Rate limiting
- Limites **RPM** (requêtes par minute) et **TPM** (tokens par minute)
- Appliqué par clé virtuelle
- Persistance SQLite ou PostgreSQL
- Atomic check-and-increment (pas de race conditions)

### Budgets
- Limites budgétaires en USD par clé virtuelle
- Périodes de reset configurables : 30s, 5min, 1h, 1j, 1sem, 1mois, 1an
- Enforcement via pre-hook plugin

### Authentification administrateur
- Protection de toutes les routes de management par clé admin (`PYLOS_ADMIN_KEY`)
- Comparaison en temps constant (constant-time)

### OIDC / JWT
- Validation des tokens JWT (RS256 et HS256)
- Support de n'importe quel fournisseur OIDC

---

## 10. Observabilité

### Métriques Prometheus
Endpoint `/metrics` exposant :
- Compteurs de requêtes (par fournisseur, modèle, statut)
- Histogrammes de latence
- Compteurs de tokens (prompt, completion)
- Requêtes en cours (in-flight)
- Time-to-First-Token (TTFT)
- Tokens par seconde

### Tracing distribué (OpenTelemetry)
- Export OTLP HTTP
- Conventions sémantiques `gen_ai.system` par fournisseur

### Logging structuré
- Requête/réponse complète : fournisseur, modèle, latence, tokens, coût USD, statut, clé virtuelle
- Stockage : **SQLite** (WAL mode) ou **PostgreSQL**
- API de requêtage avec filtres, statistiques, histogrammes temporels

### Dashboards Grafana
- Dashboard pré-construit avec provisioning automatique
- Stack complète Docker : Pylos + UI + Prometheus + Grafana

---

## 11. Administration (Interface React)

### Pages de l'interface

| Page | Fonctionnalités |
|---|---|
| **Dashboard** | KPIs (requêtes, succès, latence, tokens, coût), graphiques d'activité, période 1h/6h/24h/7d/30d, auto-refresh 30s |
| **Playground** | Chat interactif, sélecteur de modèle groupé par fournisseur, comparaison A/B, métriques temps réel, export |
| **Logs** | Journal paginé avec filtres (période, fournisseur, statut, clé, modèle), modal détaillé, auto-refresh 10s |
| **Analytics** | Analyses par fournisseur : volume, heatmap latence, coût comparé, économies estimées vs GPT-4o |
| **Providers** | CRUD fournisseurs, clés multiples pondérées, test de connectivité |
| **Virtual Keys** | CRUD clés, ACL fournisseurs/modèles, budget, rate limits, expiration |
| **Model Catalog** | Registre des modèles, prix, fenêtre de contexte, capacités (vision, tools, streaming) |
| **Guardrails** | Activation/désactivation des guardrails, configuration PII, mots-clés |
| **Budgets & Billing** | Barres d'utilisation budgétaire par clé, seuils colorés (vert/jaune/rouge) |
| **Organizations** | Gestion multi-tenant des organisations |
| **Teams** | Gestion des équipes au sein des organisations |
| **Internal Users** | Utilisateurs avec rôles et appartenances |
| **Access Groups** | Groupes d'accès avec permissions modèles/fournisseurs |
| **Policies** | Politiques configurables (JSON) |
| **Tool Policies** | Politiques d'accès par outil MCP |
| **Search Tools** | Configuration des outils de recherche MCP |
| **Vector Stores** | Configuration des stores vectoriels MCP |

---

## 12. Gestion des accès (RBAC multi-tenant)

### Hiérarchie
```
Organizations → Teams → Users → Access Groups → Policies
```

### Fonctionnalités
- **Multi-tenant** : organisations isolées avec leurs équipes et utilisateurs
- **Groupes d'accès** : permissions sur des modèles et fournisseurs spécifiques
- **Tool policies** : contrôle granulaire des outils MCP (modèles autorisés, rate limits par outil)
- **Politiques JSON** : flexibilité totale pour des règles custom

---

## 13. MCP (Model Context Protocol)

- **Proxy MCP** : interface unifiée pour les outils et stores vectoriels
- **Search Tools** : interface de configuration des outils de recherche
- **Vector Stores** : interface de configuration des stores vectoriels
- **Tool Policies** : gestion des politiques d'accès par outil avec API CRUD
- **Rate limiting MCP** : vérification des quotas par clé virtuelle sur les endpoints MCP
- **ACL MCP** : contrôle d'accès basé sur `virtual_key_id` et `team_id` (logique AND quand les deux sont définis)

---

## 14. Persistance des données

### Bases de données (SQLite)
- `pylos-logs.db` — Logs de requêtes
- `pylos-catalog.db` — Catalogue de modèles
- `pylos-budget.db` — Budgets
- `pylos-ratelimit.db` — Rate limits
- `pylos-virtualkeys.db` — Clés virtuelles
- `pylos-prompts.db` — Prompts templates
- `pylos-config.db` — Configuration

### PostgreSQL
- Base unique remplaçant tous les stores SQLite
- Configurable via `database_url`

### Vector store
- **Qdrant** pour le cache sémantique, le RAG et les stores vectoriels MCP

### Graph store
- **Memgraph** (compatible neo4j) pour la mémoire cross-agent

---

## 15. Configuration

- Fichier unique **`pylos.json`** avec schéma JSON
- Références à des variables d'environnement (`env.VAR_NAME`)
- **Hot-reload** : rechargement à chaud via `POST /config/reload` ou bouton UI
- **Versioning** : format v2 avec sémantique deny-all par défaut
- **SHA-256 hashing** pour détection de changement
- CRUD runtime des providers et clés virtuelles

---

## 16. Déploiement

### Docker
- Image multi-arch (amd64 + arm64)
- Build multi-stage cross-compilé
- Stack complète : `docker-compose up`

### Kubernetes
- **Helm chart** complet dans `helm/pylos/`
- Compatible ArgoCD / GitOps

### CI/CD
- GitHub Actions : lint, test, build, push GHCR
- **Promotion dev→prod** : endpoint `POST /api/github/promote`

---

## 17. Stack technique

### Backend (Rust)
- **Web** : Axum 0.7 + Tokio + Tower
- **Base de données** : SQLx 0.8 (SQLite + PostgreSQL), Rusqlite
- **Vector store** : Qdrant API REST
- **Graph store** : neo4rs (Memgraph)
- **Cache** : Moka (cache mémoire haute performance)
- **Observabilité** : Prometheus, OpenTelemetry OTLP, Tracing
- **Cloud** : AWS SDK (Bedrock, STS), Azure SDK, Kube
- **Sécurité** : Ring, jsonwebtoken, regex

### Frontend (TypeScript)
- **UI** : React 19, React Router 7, TailwindCSS 3
- **État** : TanStack Query 5
- **Graphiques** : Recharts, date-fns
- **Build** : Vite 8, TypeScript 6
- **Monitoring** : OpenObserve RUM

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   pylos-server                       │
│           Axum HTTP server, routes, middleware       │
├─────────────────────────────────────────────────────┤
│                 pylos-application                     │
│     Use cases, orchestration, stores, plugins        │
├─────────────────────────────────────────────────────┤
│               pylos-infrastructure                   │
│      Provider adapters (OpenAI, Anthropic, etc.)     │
├─────────────────────────────────────────────────────┤
│                   pylos-core                         │
│         Domain entities, traits, config types        │
└─────────────────────────────────────────────────────┘
```

Architecture hexagonale (ports & adapters) en 4 crates Rust.

---

## Résumé des Économies

| Mécanisme | Économie | Effort |
|---|---|---|
| Cache sémantique | 60-80% sur requêtes répétitives | Configuration uniquement |
| Prefix cache | 40-60% sur TTFT + cache KV | Automatique |
| Cache aligner | 30-50% sur tokens de cache miss | Automatique |
| Batching async | 50% sur requêtes différées | Configuration mode |
| Compression caveman | 20-40% tokens par requête | Automatique |
| Model mapping | 30-70% sur coût unitaire | Configuration règles |
| **Cumul possible** | **~80-90%** | **Combiné** |

## Points clés pour la présentation vidéo

1. **Économies LLM** : Jusqu'à 90% de réduction sur la facture grâce au caching multicouche (sémantique, préfixe, alignement), au batching intelligent et au model mapping
2. **RAG avancé** : Pipeline complet avec expansion de requêtes, HyDE, parent-child chunking, et CRAG avec fallback web — zéro hallucination par manque de contexte
3. **Mémoire persistante** : Graphe de connaissances Memgraph qui suit les préférences utilisateur à travers les sessions et les agents — même en streaming
4. **Gouvernance** : Contrôle d'accès granulaire, rate limiting, budgets — indispensable pour les équipes
5. **Observabilité** : Métriques, tracing, logs — tout est traçable et mesurable
6. **Rust** : Performance, sécurité mémoire, faible latence
7. **Plugins** : Architecture extensible pour la sécurité, le caching, le RAG
8. **Déploiement** : Docker, Kubernetes, CI/CD prêts à l'emploi
9. **Interface React** : Dashboard complet pour l'administration sans ligne de commande
