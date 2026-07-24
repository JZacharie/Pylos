# Pylos Configuration Guide (`pylos.json`)

Pylos is configured via a single, hot-reloadable JSON file (typically `pylos.json`). This file defines the server runtime options, active LLM providers, virtual key governance, budgets, rate limits, and plugin features.

## Schema & Format

The configuration supports:

- **Environment Reference Resolution**: Any string matching `"env.VAR_NAME"` is dynamically resolved to the value of the environment variable `VAR_NAME` (useful for API keys and database credentials).
- **Human-Readable Durations**: Strings representing time windows (e.g. `30s`, `5m`, `1h`, `1d`, `1w`, `1M`, `1Y`) are parsed into seconds.

---

## Configuration Reference

### 1. Root Options

```json
{
  "$schema": "https://pylos.ai/schema",
  "version": 2,
  "server": { ... },
  "providers": { ... },
  "governance": { ... },
  "plugins": [ ... ]
}
```

- **`$schema`** (string): URI to the JSON validation schema.
- **`version`** (integer): Configuration format version. Version `2` treats empty model/key arrays as "deny-all" for extra safety.

---

### 2. Server Configuration (`server`)

Controls the HTTP network interface, logging, database connections, and request queuing behavior.

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `host` | string | `"0.0.0.0"` | Listen host address. |
| `port` | integer | `3000` | Listen port. |
| `log_level` | string | `"info"` | Log level (`error`, `warn`, `info`, `debug`, `trace`). |
| `enable_logging` | boolean | `true` | Enables HTTP request/response logging. |
| `disable_content_logging` | boolean | `false` | If `true`, hides message body content in logs for privacy. |
| `log_retention_days` | integer | `365` | Logs retention threshold. |
| `max_request_body_size_mb` | integer | `100` | Max request payload size. |
| `allowed_origins` | string[] | `["*"]` | Allowed CORS origins. |
| `enforce_auth_on_inference` | boolean | `false` | Require valid authentication header even for public endpoints. |
| `log_db_path` | string | *None* | Path to local SQLite file for logs. If omitted, uses in-memory logs (up to 10k items). |
| `database_url` | string/EnvRef | *None* | PostgreSQL connection string (e.g. `env.DATABASE_URL`). If defined, PostgreSQL replaces local SQLite files. |
| `queuing` | object | See below | Concurrency and queue timeout policies. |

#### Queuing Configuration (`server.queuing`)

- **`max_concurrency`** (integer, default: `100`): Max concurrent active HTTP requests.
- **`max_queue_size`** (integer, default: `1000`): Max size of the request queue when concurrency limit is reached.
- **`queue_timeout_ms`** (integer, default: `30000`): Timeout duration in milliseconds for a request waiting in the queue.

---

### 3. Providers Configuration (`providers`)

Maps unique provider IDs to their respective keys, routing weights, and endpoints. Pylos maps generic SDK targets to these providers.

```json
"providers": {
  "openai": {
    "keys": [
      {
        "name": "primary",
        "value": "env.OPENAI_API_KEY",
        "models": ["*"],
        "weight": 1.0
      }
    ],
    "network": {
      "timeout_secs": 30,
      "max_retries": 3
    }
  }
}
```

#### Provider Properties

- **`keys`** (array): Upstream API keys.
  - **`name`** (string): Unique identifier for the key.
  - **`value`** (string/EnvRef): Literal API key or an env var reference (`"env.VAR"`).
  - **`models`** (string[]): Models permitted for this key (use `["*"]` for all).
  - **`weight`** (float, default: `1.0`): Weight used for load-balancing requests when multiple keys are present.
  - **`bedrock_key_config`** (object, optional): AWS IAM Credentials config (see below).
  - **`azure_config`** (object, optional): Azure OpenAI endpoint mappings.
- **`network`** (object): Network policies.
  - **`base_url`** (string, optional): Target HTTP endpoint override (e.g., local Ollama/vLLM URL).
  - **`timeout_secs`** (integer, default: `30`): Request timeout.
  - **`max_retries`** (integer, default: `3`): Max retries for failures.
  - **`retry_backoff_initial_ms`** (integer, default: `100`): Initial backoff duration.
  - **`retry_backoff_max_ms`** (integer, default: `5000`): Max backoff limit.
  - **`extra_headers`** (object): Additional HTTP headers to attach to upstream requests.
- **`concurrency`** (object):
  - **`concurrency`** (integer, default: `100`): Max concurrent requests to this provider.
  - **`buffer_size`** (integer, default: `1000`): Worker task queue size.

#### Special Provider: AWS Bedrock (`bedrock_key_config`)

For AWS Bedrock authentication, you can configure access key/secret pairs or default to IAM roles (IRSA) by omitting keys:

```json
{
  "access_key_id": "env.AWS_ACCESS_KEY_ID",
  "secret_access_key": "env.AWS_SECRET_ACCESS_KEY",
  "region": "us-east-1",
  "role_arn": "env.AWS_ROLE_ARN",
  "role_session_name": "pylos-session"
}
```

---

### 4. Governance Configuration (`governance`)

Manages Virtual Keys, budgets, rate limiting rules, and routing decisions.

#### Virtual Keys (`governance.virtual_keys`)

Virtual keys (`sk-pylos-*`) partition access.

- **`id`** (string): Unique identifier.
- **`name`** (string): Friendly name.
- **`description`** (string, optional): Context description.
- **`value`** (string/EnvRef, optional): Raw key value (auto-generated if omitted).
- **`is_active`** (boolean, default: `true`): Active state.
- **`rate_limit_id`** (string, optional): ID of the rate limit config to apply.
- **`provider_configs`** (array): Target permissions:
  - **`provider`** (string): Upstream provider ID.
  - **`allowed_models`** (string[]): Models allowed (use `["*"]` for all).
  - **`key_names`** (string[]): Upstream key names allowed.

#### Budgets (`governance.budgets`)

Limits USD spend over a specific sliding window.

- **`id`** (string): Budget identifier.
- **`max_limit`** (float): Max spend allowed in USD.
- **`reset_duration`** (Duration): Window duration (e.g. `"1d"`, `"1M"`).
- **`virtual_key_id`** (string, optional): Scope limit to a specific virtual key.

#### Rate Limits (`governance.rate_limits`)

Controls token and request frequencies.

- **`id`** (string): Rate limit identifier.
- **`token_max_limit`** (integer): Token limit (0 = unlimited).
- **`token_reset_duration`** (Duration): Token window.
- **`request_max_limit`** (integer): Request limit (0 = unlimited).
- **`request_reset_duration`** (Duration): Request window.

#### Routing Rules (`governance.routing_rules`)

CEL-based dynamic router.

- **`cel_expression`** (string): CEL condition (e.g. `request.model == 'gpt-4'`).
- **`targets`** (array):
  - **`provider`** (string): Target provider.
  - **`model`** (string): Target model override.
  - **`weight`** (float): Random allocation weight.
- **`fallbacks`** (string[]): Chain of backup providers if primary target fails.

---

### 5. Plugins Configuration (`plugins`)

Enables and configures plugins like telemetry, caching, memory graph, and guardrails.

```json
"plugins": [
  {
    "name": "semantic_cache",
    "enabled": true,
    "config": {
      "similarity_threshold": 0.92,
      "ttl_secs": 86400
    }
  }
]
```

Standard Plugins:

- **`telemetry`**: Prometheus/OTel exporter configurations.
- **`logging`**: Raw request logging config.
- **`semantic_cache`**: Vector-similarity caching (Cosine similarity).
- **`mem0`**: Memory graph storage.
- **`guardrails`**: PII masking and safety keywords.
