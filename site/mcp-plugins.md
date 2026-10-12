---
layout: default
title: MCP & Plugins
description: Proxy Model Context Protocol dynamique et architecture de plugins pré/post-traitement dans Pylos.
permalink: /mcp-plugins/
---

# Proxy Model Context Protocol (MCP) & Plugins

Pylos fait le pont entre vos modèles de langage et vos outils externes grâce à son support natif du protocole **MCP (Model Context Protocol)** et son architecture de plugins modulaires.

---

## 🔌 1. Le Proxy Dynamique MCP

Le protocole **Model Context Protocol (MCP)** est un standard ouvert permettant d'exposer de manière normalisée des serveurs d'outils, des contextes de fichiers et des ressources dynamiques aux modèles d'IA.

### Rôle de Pylos comme Proxy MCP :
1. **Agrégation multi-serveurs** : Pylos fédère plusieurs serveurs d'outils (ex: base de données, API GitHub, terminal sécurisé, connecteurs internes) derrière une interface unique.
2. **Cloisonnement & Sécurité** : Seules les clés virtuelles autorisées peuvent appeler des outils spécifiques (principe du moindre privilège).
3. **Audit & Traçabilité** : Chaque invocation d'outil par un agent est consignée avec ses paramètres d'entrée et de sortie dans les journaux d'audit de Pylos.

```
Agent IA (Claude / GPT) ──► Pylos (MCP Proxy) ──► Serveur MCP Git
                                               ├──► Serveur MCP PostgreSQL
                                               └──► Serveur MCP Filesystem
```

---

## 🧩 2. Architecture de Plugins Rust (`LlmPlugin`)

Le crate `pylos-core` définit un trait extensible permettant d'intercepter chaque requête à différentes étapes de son cycle de vie :

```rust
#[async_trait]
pub trait LlmPlugin: Send + Sync {
    /// Nom unique du plugin
    fn name(&self) -> &'static str;

    /// Crochet exécuté avant la vérification de cache et l'appel LLM
    async fn on_request(&self, ctx: &mut RequestContext) -> Result<(), PylosError>;

    /// Crochet exécuté après la réponse du LLM
    async fn on_response(&self, ctx: &mut ResponseContext) -> Result<(), PylosError>;
}
```

---

## 🛡️ 3. Plugins Officiels Intégrés

### Masquage PII (`pii_masker`)
Détecte et anonymise à la volée les numéros de sécurité sociale, les cartes bancaires, les adresses emails et numéros de téléphone avant que les prompts ne quittent votre réseau interne vers des fournisseurs tiers.

### Garde-fous de Contenu (`content_guardrails`)
Analyse la conformité éthique et la sécurité des requêtes entrantes pour bloquer les tentatives de contournement (*jailbreak*) ou les instructions malveillantes.

### Injecteur de Contexte (`context_enricher`)
Injecte automatiquement des métadonnées contextuelles (date courante, fuseau horaire de l'utilisateur, profil d'organisation) sans surcharger le code client de vos applications.

---

## ⚙️ Configuration dans `pylos.json`

Les plugins sont activés et configurés directement dans la section `"plugins"` :

```json
"plugins": [
  {
    "type": "pii_masker",
    "enabled": true,
    "rules": {
      "mask_emails": true,
      "mask_credit_cards": true,
      "mask_phone_numbers": true
    }
  },
  {
    "name": "guardrails",
    "enabled": true,
    "config": {
      "mode": "block",
      "blocked_keywords": ["internal_secret", "confidential"],
      "moderation_threshold": 0.7
    }
  },
  {
    "type": "mcp_proxy",
    "enabled": true,
    "servers": [
      {
        "name": "internal-db",
        "transport": "sse",
        "endpoint": "http://mcp-db.internal:8000/sse"
      }
    ]
  }
]
```

> ℹ️ **Mode Bloquant & Codes HTTP** : En mode `"block"`, toute violation (mot-clé interdit, injection de prompt, score de modération dépassé) renvoie immédiatement un code **HTTP 400 Bad Request** (`content_policy_violation`). Une normalisation Unicode anti-évasion (espaces et séparateurs invisibles) est appliquée. Les règles statiques de filtrage restent heuristiques et complémentaires d'une solution ML dédiée.

