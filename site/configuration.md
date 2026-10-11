---
layout: default
title: Configuration (pylos.json)
description: Guide de référence complet du fichier de configuration centralisé pylos.json.
permalink: /configuration/
---

# Guide de Configuration (`pylos.json`)

Pylos est piloté par un unique fichier de configuration centralisé : `pylos.json`. Ce fichier définit les paramètres du serveur HTTP, les fournisseurs d'IA, les règles de gouvernance budgétaire et les extensions actives.

---

## ⚡ Principes Fondamentaux

### Résolution Dynamique des Variables d'Environnement
Toute chaîne préfixée par `"env."` est résolue dynamiquement à partir des variables d'environnement de votre système ou de votre fichier `.env`. Cela évite d'exposer des secrets en clair dans votre configuration :

```json
"value": "env.OPENAI_API_KEY"
```

### Durées Lisibles par l'Humain
Les intervalles de temps peuvent être exprimés avec des unités explicites :
* `30s` (30 secondes)
* `5m` (5 minutes)
* `1h` (1 heure)
* `1d` (1 jour)
* `1w` (1 semaine)
* `1M` (1 mois)

---

## 📄 Structure Globale

```json
{
  "$schema": "https://pylos.ai/schema/v2.json",
  "version": 2,
  "server": { ... },
  "providers": { ... },
  "governance": { ... },
  "plugins": [ ... ]
}
```

---

## 1. Section Serveur (`server`)

Contrôle les interfaces réseau, les niveaux de journalisation et la persistance :

```json
"server": {
  "host": "0.0.0.0",
  "port": 3000,
  "log_level": "info",
  "enable_logging": true,
  "disable_content_logging": false,
  "log_retention_days": 90,
  "max_request_body_size_mb": 50,
  "allowed_origins": ["*"],
  "enforce_auth_on_inference": true,
  "log_db_path": "./pylos-logs.db",
  "database_url": "env.DATABASE_URL",
  "queuing": {
    "max_concurrency": 250,
    "max_queue_size": 2000,
    "queue_timeout_ms": 30000
  }
}
```

| Paramètre | Type | Défaut | Description |
| :--- | :--- | :--- | :--- |
| `host` | string | `"0.0.0.0"` | Adresse IP d'écoute du serveur. |
| `port` | integer | `3000` | Port TCP d'écoute HTTP. |
| `log_level` | string | `"info"` | Niveau de détail : `error`, `warn`, `info`, `debug`, `trace`. |
| `disable_content_logging` | boolean | `false` | Si activé, masque le contenu des prompts/réponses pour la conformité RGPD. |
| `enforce_auth_on_inference` | boolean | `true` | Exige un en-tête `Authorization: Bearer sk-pylos-*` valide. |
| `log_db_path` | string | `null` | Emplacement du fichier SQLite local pour les logs et statistiques. |
| `database_url` | string | `null` | URL PostgreSQL pour un déploiement multi-instances scalé. |
| `queuing.max_concurrency` | integer | `100` | Nombre maximal de requêtes traitées simultanément en vol. |
| `queuing.max_queue_size` | integer | `1000` | Capacité de la file d'attente en cas de pic de trafic. |

---

## 2. Section Fournisseurs (`providers`)

Définit les clés d'API sources, la pondération pour la répartition de charge et les timeouts réseau :

```json
"providers": {
  "openai": {
    "keys": [
      {
        "name": "primary",
        "value": "env.OPENAI_API_KEY",
        "models": ["gpt-4o", "gpt-4o-mini", "text-embedding-3-small"],
        "weight": 1.0
      }
    ],
    "network": {
      "timeout_secs": 30,
      "max_retries": 3
    }
  },
  "anthropic": {
    "keys": [
      {
        "name": "prod",
        "value": "env.ANTHROPIC_API_KEY",
        "models": ["claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022"],
        "weight": 1.0
      }
    ],
    "network": {
      "timeout_secs": 45,
      "max_retries": 2
    }
  },
  "aws-bedrock": {
    "keys": [
      {
        "name": "eu-west-3",
        "value": "env.AWS_ACCESS_KEY_ID",
        "models": ["anthropic.claude-3-sonnet-20240229-v1:0"],
        "weight": 0.5
      }
    ]
  }
}
```

---

## 3. Section Gouvernance (`governance`)

Permet de créer des clés virtuelles cloisonnées par équipe ou projet avec des plafonds de budget et des limites de requêtes :

```json
"governance": {
  "default_budget": {
    "amount": 50.0,
    "currency": "USD",
    "period": "1M"
  },
  "virtual_keys": [
    {
      "key": "sk-pylos-marketing-team",
      "name": "Marketing Team Key",
      "team": "Marketing",
      "models": ["gpt-4o-mini", "claude-3-5-haiku-20241022"],
      "rate_limits": {
        "rpm": 120,
        "tpm": 150000
      },
      "budget": {
        "amount": 100.0,
        "currency": "USD",
        "period": "1M"
      }
    }
  ]
}
```

---

## 4. Rechargement sans Interruption

Toute modification de `pylos.json` peut être appliquée instantanément sans redémarrer le conteneur ni couper les connexions en cours :

```bash
curl -X POST http://localhost:3000/config/reload \
  -H "Authorization: Bearer $PYLOS_ADMIN_KEY"
```
