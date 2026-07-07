# Référence complète — pylos.json

Ce document décrit tous les champs acceptés dans le fichier de configuration `pylos.json`.

---

## Structure racine

```json
{
  "$schema": "...",
  "version": 2,
  "server": {},
  "providers": {},
  "governance": {},
  "plugins": []
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `$schema` | string | — | URL du schema JSON (optionnel, pour la validation IDE) |
| `version` | number | `2` | Version du format. v2 = `models: []` signifie deny-all |
| `server` | object | voir ci-dessous | Configuration du serveur HTTP |
| `providers` | object | `{}` | Map de providers LLM (clé = nom du provider) |
| `governance` | object | voir ci-dessous | Virtual keys, budgets, rate limits, routing |
| `plugins` | array | `[]` | Plugins activés |

---

## server

```json
"server": {
  "port": 3000,
  "host": "0.0.0.0",
  "log_level": "info",
  "enable_logging": true,
  "disable_content_logging": false,
  "log_retention_days": 365,
  "max_request_body_size_mb": 100,
  "allowed_origins": ["*"],
  "enforce_auth_on_inference": false,
  "log_db_path": "./pylos-logs.db",
  "database_url": "env.DATABASE_URL",
  "queuing": {
    "max_concurrency": 100,
    "max_queue_size": 1000,
    "queue_timeout_ms": 30000
  }
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `port` | number | `3000` | Port d'écoute |
| `host` | string | `"0.0.0.0"` | Adresse de bind |
| `log_level` | string | `"info"` | Niveau de log : error, warn, info, debug, trace |
| `enable_logging` | bool | `true` | Activer le logging des requêtes |
| `disable_content_logging` | bool | `false` | Ne pas logger le contenu des messages (confidentialité) |
| `log_retention_days` | number | `365` | Rétention des logs en jours |
| `max_request_body_size_mb` | number | `100` | Taille max du body HTTP en MB |
| `allowed_origins` | string[] | `["*"]` | Origins CORS autorisés |
| `enforce_auth_on_inference` | bool | `false` | Exiger une virtual key sur `/v1/chat/completions` etc. |
| `log_db_path` | string | — | Chemin SQLite pour les logs. Si absent → in-memory |
| `database_url` | string | — | URL PostgreSQL. Si défini, remplace tous les SQLite. Supporte `"env.VAR"` |
| `queuing` | object | voir ci-dessous | File d'attente de requêtes |

### server.queuing

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `max_concurrency` | number | `100` | Requêtes concurrentes max |
| `max_queue_size` | number | `1000` | Taille max de la file d'attente |
| `queue_timeout_ms` | number | `30000` | Timeout d'attente en file (ms) |

---

## providers

Map clé-valeur où la clé est le nom du provider (libre) et la valeur est un objet `ProviderConfig`.

```json
"providers": {
  "ollama": {
    "keys": [...],
    "network": {...},
    "concurrency": {...}
  },
  "openai": {...},
  "bedrock": {...}
}
```

### providers.\<name\>.keys[]

Chaque provider a un tableau de clés API :

```json
{
  "name": "default",
  "value": "sk-xxx",
  "models": ["*"],
  "weight": 1.0,
  "bedrock_key_config": null,
  "azure_config": null
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `name` | string | requis | Identifiant interne de la clé |
| `value` | string | `""` | Clé API. Supporte `"env.VAR_NAME"` pour résolution dynamique |
| `models` | string[] | `["*"]` | Modèles autorisés. `["*"]` = tous. `[]` = deny-all (v2) |
| `weight` | number | `1.0` | Poids pour le load-balancing pondéré |
| `bedrock_key_config` | object | — | Config AWS Bedrock (voir ci-dessous) |
| `azure_config` | object | — | Config Azure OpenAI (voir ci-dessous) |

### providers.\<name\>.network

```json
{
  "base_url": "http://100.104.35.121:11434/v1",
  "timeout_secs": 30,
  "max_retries": 3,
  "retry_backoff_initial_ms": 100,
  "retry_backoff_max_ms": 5000,
  "extra_headers": {}
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `base_url` | string | — | URL de base du provider (requis pour Ollama, vLLM, etc.) |
| `timeout_secs` | number | `30` | Timeout par requête en secondes |
| `max_retries` | number | `3` | Nombre de retries sur erreur |
| `retry_backoff_initial_ms` | number | `100` | Backoff initial entre retries (ms) |
| `retry_backoff_max_ms` | number | `5000` | Backoff max entre retries (ms) |
| `extra_headers` | object | `{}` | Headers additionnels envoyés au provider |

### providers.\<name\>.concurrency

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `concurrency` | number | `100` | Workers concurrents max vers ce provider |
| `buffer_size` | number | `1000` | Taille du buffer de la queue |

### bedrock_key_config (pour le provider "bedrock")

```json
{
  "access_key_id": "env.AWS_ACCESS_KEY_ID",
  "secret_access_key": "env.AWS_SECRET_ACCESS_KEY",
  "session_token": "env.AWS_SESSION_TOKEN",
  "region": "us-east-1",
  "role_arn": "arn:aws:iam::123456789012:role/BedrockRole",
  "external_id": null,
  "role_session_name": "pylos-session"
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `access_key_id` | string | — | AWS Access Key. Si absent → chaîne de credentials par défaut |
| `secret_access_key` | string | — | AWS Secret Key |
| `session_token` | string | — | STS Session Token (credentials temporaires) |
| `region` | string | `"us-east-1"` | Région AWS Bedrock |
| `role_arn` | string | — | ARN du rôle IAM à assumer via STS |
| `external_id` | string | — | External ID pour l'AssumeRole cross-account |
| `role_session_name` | string | `"pylos-session"` | Nom de la session STS |

### azure_config (pour le provider "azure")

```json
{
  "resource_name": "my-azure-resource",
  "deployment_name": "gpt-4o",
  "api_version": "2024-02-01"
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `resource_name` | string | requis | Nom de la ressource Azure (`{name}.openai.azure.com`) |
| `deployment_name` | string | requis | Nom du déploiement Azure |
| `api_version` | string | `"2024-02-01"` | Version de l'API Azure OpenAI |

---

## governance

```json
"governance": {
  "virtual_keys": [],
  "budgets": [],
  "rate_limits": [],
  "routing_rules": []
}
```

### governance.virtual_keys[]

Clés virtuelles distribuées aux utilisateurs/équipes :

```json
{
  "id": "vk-team-frontend",
  "name": "Équipe Frontend",
  "description": "Clé pour l'équipe frontend",
  "value": "sk-pylos-xxx",
  "is_active": true,
  "rate_limit_id": "rl-standard",
  "provider_configs": [
    {
      "provider": "openai",
      "allowed_models": ["gpt-4o", "gpt-4o-mini"],
      "key_names": ["*"],
      "weight": 1.0
    }
  ],
  "team_alias": "frontend",
  "team_id": "team-001",
  "organization_id": "org-001",
  "access_group_id": "ag-001",
  "user_email": "dev@example.com",
  "expires_at": 1735689600000
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `id` | string | requis | Identifiant unique (doit être unique globalement) |
| `name` | string | requis | Nom lisible |
| `description` | string | — | Description |
| `value` | string | — | Valeur de la clé. Préfixe `sk-pylos-` auto-ajouté |
| `is_active` | bool | `true` | Actif/désactivé |
| `rate_limit_id` | string | — | ID du rate limit associé |
| `provider_configs` | array | `[]` | Providers/modèles autorisés pour cette clé |
| `team_alias` | string | — | Alias de l'équipe |
| `team_id` | string | — | ID de l'équipe |
| `organization_id` | string | — | ID de l'organisation |
| `access_group_id` | string | — | Groupe d'accès |
| `user_email` | string | — | Email de l'utilisateur propriétaire |
| `user_id` | string | — | ID utilisateur |
| `created_at` | number | — | Timestamp de création (ms) |
| `created_by` | string | — | Créateur |
| `updated_at` | number | — | Dernière modification (ms) |
| `last_active` | number | — | Dernière utilisation (ms) |
| `expires_at` | number | — | Expiration (ms). `null` = pas d'expiration |

### governance.virtual_keys[].provider_configs[]

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `provider` | string | requis | Nom du provider (`"openai"`, `"ollama"`, `"*"`) |
| `allowed_models` | string[] | `["*"]` | Modèles autorisés pour cette VK sur ce provider |
| `key_names` | string[] | `["*"]` | Clés provider autorisées |
| `weight` | number | `1.0` | Poids de routing |

### governance.budgets[]

```json
{
  "id": "budget-frontend",
  "max_limit": 100.0,
  "reset_duration": "1M",
  "current_usage": 0.0,
  "virtual_key_id": "vk-team-frontend"
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `id` | string | requis | Identifiant unique |
| `max_limit` | number | requis | Budget max en USD |
| `reset_duration` | string | requis | Période de reset : `"30s"`, `"5m"`, `"1h"`, `"1d"`, `"1w"`, `"1M"`, `"1Y"` |
| `current_usage` | number | `0.0` | Usage courant (mis à jour par Pylos) |
| `virtual_key_id` | string | — | VK associée |

### governance.rate_limits[]

```json
{
  "id": "rl-standard",
  "token_max_limit": 100000,
  "token_reset_duration": "1h",
  "request_max_limit": 60,
  "request_reset_duration": "1m"
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `id` | string | requis | Identifiant unique |
| `token_max_limit` | number | `0` | Tokens max par fenêtre (0 = illimité) |
| `token_reset_duration` | string | — | Période de reset pour les tokens |
| `request_max_limit` | number | `0` | Requêtes max par fenêtre (0 = illimité) |
| `request_reset_duration` | string | — | Période de reset pour les requêtes |

### governance.routing_rules[]

Règles de routage avancé basées sur des expressions CEL :

```json
{
  "id": "rule-gpt4-to-deepseek",
  "name": "Redirect GPT-4 to DeepSeek",
  "enabled": true,
  "cel_expression": "request.model.startsWith('gpt-4')",
  "targets": [
    {"provider": "deepseek", "model": "deepseek-v4-pro", "weight": 1.0}
  ],
  "fallbacks": ["openai"],
  "priority": 10
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `id` | string | requis | Identifiant unique |
| `name` | string | requis | Nom lisible |
| `enabled` | bool | `true` | Actif/désactivé |
| `cel_expression` | string | requis | Expression CEL pour matcher les requêtes |
| `targets` | array | requis | Cibles de routing avec poids |
| `fallbacks` | string[] | `[]` | Providers fallback si les cibles échouent |
| `priority` | number | `0` | Priorité (plus petit = évalué en premier) |

#### routing_rules[].targets[]

| Champ | Type | Description |
|---|---|---|
| `provider` | string | Provider cible |
| `model` | string | Modèle override (null = garder celui de la requête) |
| `weight` | number | Poids pour routing probabiliste (doit être > 0) |

---

## plugins[]

```json
"plugins": [
  {"name": "mem0", "enabled": true},
  {"name": "memory", "enabled": true},
  {"name": "semantic_cache", "enabled": true, "config": {...}},
  {"name": "otel", "enabled": true, "config": {...}},
  {"name": "guardrails", "enabled": true}
]
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `name` | string | requis | Nom du plugin (voir tableau ci-dessous) |
| `enabled` | bool | `true` | Actif/désactivé |
| `config` | object | `{}` | Configuration spécifique au plugin (JSON libre) |

### Plugins disponibles

| Nom | Dépendance externe | Description |
|---|---|---|
| `mem0` | Sidecar Mem0 (port 7577) | Mémoire conversationnelle via Mem0 |
| `memory` | Memgraph (port 7687) | Graphe de connaissances (triplets entités-relations) |
| `semantic_cache` | Qdrant (port 6333) | Cache sémantique par similarité vectorielle |
| `otel` | Collecteur OTLP | Tracing distribué OpenTelemetry |
| `guardrails` | Rien | Masquage PII, blocage contenu sensible |

### Config spécifique : semantic_cache

```json
{
  "name": "semantic_cache",
  "enabled": true,
  "config": {
    "collection_name": "pylos_semantic_cache",
    "embedding_model": "nomic-embed-text-v2-moe-GGUF",
    "similarity_threshold": 0.92,
    "ttl_secs": 86400
  }
}
```

| Champ | Type | Défaut | Description |
|---|---|---|---|
| `collection_name` | string | `"pylos_semantic_cache"` | Nom de la collection Qdrant |
| `embedding_model` | string | `"nomic-embed-text-v2-moe-GGUF"` | Modèle d'embedding à utiliser |
| `similarity_threshold` | number | `0.9` | Seuil de similarité pour un cache hit (0.0 à 1.0) |
| `ttl_secs` | number | `86400` | Durée de vie des entrées en cache (secondes) |

---

## Syntaxe EnvVar

Partout où une valeur supporte `EnvVar`, tu peux écrire :

- `"sk-test-123"` → valeur littérale
- `"env.OPENAI_API_KEY"` → résolution de la variable d'environnement `$OPENAI_API_KEY` au démarrage

---

## Syntaxe Duration

Les champs de type Duration acceptent une chaîne avec unité :

| Format | Signification |
|---|---|
| `"30s"` | 30 secondes |
| `"5m"` | 5 minutes |
| `"1h"` | 1 heure |
| `"1d"` | 1 jour |
| `"1w"` | 1 semaine |
| `"1M"` | 1 mois (~30 jours) |
| `"1Y"` | 1 an (~365 jours) |

---

## Exemple complet

```json
{
  "version": 2,
  "server": {
    "port": 3000,
    "log_level": "info",
    "enable_logging": true,
    "enforce_auth_on_inference": true
  },
  "providers": {
    "ollama": {
      "keys": [{"name": "local", "value": "ollama", "models": ["*"]}],
      "network": {"base_url": "http://100.104.35.121:11434/v1", "timeout_secs": 120}
    },
    "openai": {
      "keys": [{"name": "default", "value": "env.OPENAI_API_KEY", "models": ["gpt-4o", "gpt-4o-mini"]}],
      "network": {"timeout_secs": 30, "max_retries": 3}
    },
    "bedrock": {
      "keys": [{
        "name": "default",
        "value": "",
        "models": ["us.anthropic.*", "amazon.*"],
        "bedrock_key_config": {
          "region": "us-east-1",
          "role_arn": "env.AWS_ROLE_ARN"
        }
      }]
    }
  },
  "governance": {
    "virtual_keys": [
      {
        "id": "vk-dev",
        "name": "Dev Team",
        "is_active": true,
        "rate_limit_id": "rl-dev",
        "provider_configs": [
          {"provider": "*", "allowed_models": ["*"]}
        ]
      }
    ],
    "budgets": [
      {"id": "budget-dev", "max_limit": 50.0, "reset_duration": "1M", "virtual_key_id": "vk-dev"}
    ],
    "rate_limits": [
      {"id": "rl-dev", "request_max_limit": 100, "request_reset_duration": "1m", "token_max_limit": 500000, "token_reset_duration": "1h"}
    ]
  },
  "plugins": [
    {"name": "guardrails", "enabled": true},
    {"name": "semantic_cache", "enabled": true, "config": {"similarity_threshold": 0.92, "ttl_secs": 86400}},
    {"name": "mem0", "enabled": true}
  ]
}
```
