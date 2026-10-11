---
layout: default
title: Architecture Système
description: Découverte de l'architecture hexagonale en Rust, des flux de requêtes d'inférence et des mécanismes de résilience de Pylos.
---

# Architecture & Principes de Conception

Pylos a été entièrement conçu selon les principes de l'**Architecture Hexagonale (Ports et Adaptateurs / Clean Architecture)**. Cette structure garantit une séparation stricte des responsabilités, une testabilité maximale et une indépendance totale du cœur métier vis-à-vis des protocoles réseau et des bases de données.

---

## 🏗️ Structure en Crates Rust

Le workspace Rust de Pylos est découpé en 4 sous-crates autonomes :

```
┌─────────────────────────────────────────────────────────────────┐
│                          pylos-server                           │
│   Serveur HTTP/WS Axum, routes d'API, streaming SSE, middlewares │
├─────────────────────────────────────────────────────────────────┤
│                       pylos-application                         │
│   Orchestrateur d'inférence, ConfigStore, moteurs de quotas/budgets │
├─────────────────────────────────────────────────────────────────┤
│                     pylos-infrastructure                        │
│   Adaptateurs LLM (OpenAI, Anthropic, Bedrock), SQLx, Qdrant, Memgraph│
├─────────────────────────────────────────────────────────────────┤
│                          pylos-core                             │
│   Entités du domaine, types de config, traits (Provider, Plugin) │
└─────────────────────────────────────────────────────────────────┘
```

### 1. `pylos-core` (Domaine & Règles Métier)
* **Zéro dépendance d'E/S directe** : Ne dépend ni d'Axum, ni de Tokio, ni de SQLx.
* **Entités universelles** : Représentation unifiée des requêtes/réponses d'inférence (messages, outils, fin de complétion, usage des jetons).
* **Définition des Ports (Traits)** :
  * `Provider` : interface commune pour tout fournisseur LLM (chat, embedding, streaming).
  * `LlmPlugin` : signature des crochets d'exécution (*pre-hooks* et *post-hooks*).
  * `AuditStore` / `KeyStore` : contrats d'accès aux données.
* **Gestion centralisée des erreurs** : Enum `PylosError` couvrant les codes HTTP standard, les timeouts, les dépassements de budgets et les erreurs de fournisseurs.

### 2. `pylos-infrastructure` (Adaptateurs Externes)
* **Adaptateurs Fournisseurs LLM** :
  * `OpenAIAdapter`, `AnthropicAdapter`, `BedrockAdapter` (via AWS SDK avec prise en charge d'assume-role STS), `GeminiAdapter`, `OllamaAdapter`, etc.
* **Persistance & Données** :
  * Base de données relationnelle via `sqlx` (SQLite embarqué ou PostgreSQL multi-nœuds).
  * Cache sémantique vectoriel via le client `Qdrant`.
  * Graphe de mémoire sémantique via le protocole Bolt (`neo4rs`) connecté à `Memgraph`.

### 3. `pylos-application` (Orchestration & Cas d'Usage)
* **Orchestrateur d'Inférence** : Pilote l'exécution d'une requête de bout en bout (vérification des quotas -> caches -> plugins -> adaptateurs).
* **Moteur de Gouvernance** : Suivi des budgets en devises (USD), fenêtres glissantes et comptabilisation des tokens.
* **Routage Dynamique** : Sélection du fournisseur optimal par pondération, latence ou règles CEL (*Common Expression Language*).
* **ConfigStore** : Gestion de l'état en mémoire avec rechargement à chaud sécurisé par verrous `Arc<RwLock<Config>>`.

### 4. `pylos-server` (Couche Réseau Axum)
* **Moteur Web Asynchrone** : Axum 0.8 propulsé par Tokio.
* **Gestion du Streaming** : Transmission d'événements Server-Sent Events (SSE) avec débit continu et ultra-faible latence.
* **Middlewares** :
  * Authentification des clés virtuelles et jetons JWT.
  * Limitation de débit distribuée (Token Bucket).
  * Collecteur de métriques Prometheus (`/metrics`) et traçage OpenTelemetry.

---

## 🔄 Cycle de Vie d'une Requête d'Inférence

Chaque appel HTTP traverse une chaîne optimisée conçue pour court-circuiter le réseau dès que possible :

```
Requête Client (ex. /v1/chat/completions)
  │
  ▼
[ 1. Middleware Axum ] ─── Authentification Virtual Key + Rate-Limiting RPM/TPM
  │
  ▼
[ 2. Moteur Budget ]  ─── Contrôle des plafonds USD (fenêtre hebdomadaire/mensuelle)
  │
  ▼
[ 3. Pre-Plugins ]    ─── Masquage PII, validation de prompt, injection mémoire
  │
  ▼
[ 4. Cache TinyLFU ]  ─── Hit ? ──► [ Renvoyer réponse immédiate (0 token consommé) ]
  │ (Miss)
  ▼
[ 5. Cache Sémantique] ─── Cosine >= Seuil ? ──► [ Renvoyer réponse immédiate ]
  │ (Miss)
  ▼
[ 6. Routeur Dynamique]─── Sélection du provider (pondération, statut de santé)
  │
  ▼
[ 7. Adaptateur LLM ] ─── Appel API externe (OpenAI / Anthropic / Bedrock)
  │
  ▼
[ 8. Post-Plugins ]   ─── Extraction mémoire Memgraph, analyse de sentiment
  │
  ▼
[ 9. Audit & Métriques]── Logs SQLite/Postgres + Incrément Prometheus + SSE stream
```

---

## 🛡️ Stratégies de Résilience

Pour garantir une disponibilité de classe opérateur même en cas de panne d'un fournisseur majeur :

* **Circuit Breaker (Disjoncteur)** : Isole immédiatement un fournisseur qui retourne un taux d'erreur 5xx consécutif supérieur au seuil configuré. L'état bascule en `Open`, évitant de bloquer les threads du serveur.
* **Basculement Automatique (Fallback)** : En cas d'indisponibilité ou d'engorgement du fournisseur principal (ex: Claude 3.5 Sonnet sur Anthropic direct), la requête est immédiatement basculée de manière transparente vers un fournisseur de secours (ex: Claude sur AWS Bedrock).
* **Réessais avec Recul Exponentiel (Backoff & Jitter)** : Réessais automatiques avec variation aléatoire sur les erreurs temporaires (429 Rate Limit ou 503 Service Unavailable).
* **Gestion de File d'Attente (Queuing)** : Limite le nombre de requêtes concurrentes en vol (`max_concurrency`) et met en mémoire tampon les pics de trafic (`max_queue_size`) pendant un délai configurable (`queue_timeout_ms`).
