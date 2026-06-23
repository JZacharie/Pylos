# Pylos — Enterprise-Grade Rust LLM Gateway & MCP Proxy

<p align="center">
  <img src="logo.png" alt="Pylos Logo" width="200" />
</p>

<p align="center">
  <a href="README.md"><b>English</b></a> | 
  <a href="README.fr.md"><b>Français</b></a> | 
  <a href="README.es.md"><b>Español</b></a>
</p>

Pylos is a high-performance, ultra-low latency AI gateway written in Rust. It serves as a unified, secure proxy for 20+ LLM providers, offering a drop-in replacement for OpenAI-compatible SDKs. With built-in governance, cost management, privacy guardrails, and a sleek React admin dashboard, Pylos helps teams safely scale and monitor their AI workflows.

<p align="center">
  <img src="docs/image.png" alt="AI Gateway Overview" width="800" />
</p>

## 📖 Resources & Documentation

* **[Pylos AI Gateway Presentation (PDF)](Pylos_AI_Gateway.pdf)**
* **[Pylos AI Gateway Architecture Blueprint (PDF)](docs/Pylos_AI_Gateway_Blueprint.pdf)**

---

## 🎯 Key Benefits & Value Proposition

- **⚡ Blazing Fast Performance:** Built in Rust with async I/O (Axum & Tokio), adding `< 2ms` of overhead.
- **💰 Cost Control & Budgets:** Prevent unexpected API bills with real-time token tracking, monthly/weekly USD budgets, and rate limiting mapped to Virtual Keys.
- **🛡️ Enterprise Security & Privacy:** Google OAuth login (with static Admin Key fallback) secures the admin UI. On the data plane, real-time guardrails automatically mask PII (Personal Identifiable Information) and block unsafe content.
- **🔄 Zero-Downtime Resilience:** Automatic multi-provider fallbacks, circuit breaking, and retry strategies with exponential backoff ensure your AI integrations never fail.
- **🌐 Seamless Multi-Tenancy:** Organize users into Organizations and Teams, and allocate scoped Virtual Keys (`sk-pylos-*`) with fine-grained provider and model access controls.
- **🔌 Model Context Protocol (MCP):** Dynamic agent-to-tool capabilities via a built-in MCP server proxy.

---

## ✨ Features

- **Unified AI Gateway:** Drop-in replacement for OpenAI endpoints (`/v1/chat/completions`, `/v1/embeddings`, etc.) supporting OpenAI, Anthropic, AWS Bedrock, Google Gemini, DeepSeek, Groq, Ollama, OpenRouter, and more.
- **Intelligent Routing:** Provider-aware routing, weighted load balancing, and CEL (Common Expression Language) routing rules.
- **Strict Governance:** Scoped Virtual Keys with custom RPM/TPM limits and active budget windows.
- **Observability Built-in:** Prometheus endpoints (`/metrics`), OpenTelemetry tracing, and SQLite/Postgres logs with token-usage histograms.
- **Hot-Reloading:** Update configurations, models, and virtual keys dynamically without server restarts.
- **Modern Admin UI:** React 19 + Vite 8 dashboard to manage keys, analyze logs, and configure guardrails.
- **Caching & Token Optimization:** Built-in in-memory prefix caching (using **TinyLFU** eviction policy via `moka`) and semantic caching (using **Cosine Similarity** vector matching on **Qdrant**) to bypass LLM calls and reduce token costs.
- **Cross-Agent Memory Graph:** Long-term memory powered by **Memgraph** (via Neo4j Bolt protocol) utilizing **Cypher** queries to dynamically store and retrieve entity-relation graphs linked to Virtual Keys.
- **Built-in RAG (Retrieval-Augmented Generation):** Automatic context injection from vector collections (e.g. emails, files) stored in **Qdrant** before sending requests to downstream models.

---

## 🏗️ Architecture

Pylos uses a clean hexagonal ports-and-adapters architecture:

```
┌─────────────────────────────────────────────────────┐
│                   pylos-server                       │
│           Axum HTTP/WS server, routes, middleware    │
│├───────────────────────────────────────────────────┤│
│                 pylos-application                     │
│     Use cases, orchestration, stores, plugins        │
│├───────────────────────────────────────────────────┤│
│               pylos-infrastructure                   │
│      Provider adapters (OpenAI, Anthropic, etc.)     │
│├───────────────────────────────────────────────────┤│
│                   pylos-core                         │
│         Domain entities, traits, config types        │
└─────────────────────────────────────────────────────┘
```

---

## 🚀 Quick Start

### 1. Run Locally (Development)

```bash
# Clone the repository
git clone <repo-url> && cd Pylos

# Set up development tools & dependencies
make setup

# Create and configure env variables
cp .env.example .env
# Edit .env with your LLM provider keys and PYLOS_ADMIN_KEY

# Run the backend and UI dev servers
make run
```

### 2. Configuration (`pylos.json`)

Pylos uses a centralized `pylos.json` file for routing, governance, budgets, and plugin configurations. For a complete guide of all available configuration keys, environment variable references, and provider setups, see the [Pylos Configuration Guide](docs/pylos-config.md).

### 3. Docker Compose (Full Stack)

Start Pylos, the admin UI, Prometheus, and Grafana in one command:

```bash
docker compose up -d
```
Access the gateway at `http://localhost:3000` and the Admin Dashboard at `http://localhost:8080`.

---

## ⚡ Caching & Token Optimization

Pylos minimizes downstream LLM latencies and token expenses using two complementary caching strategies:

1. **In-Memory Prefix Cache (TinyLFU Eviction):**
   - **Algorithm:** Powered by a high-concurrency **TinyLFU** cache eviction policy via the `moka` crate.
   - **How it works:** Caches exact query representations based on model ID and prompt messages history. Subsequent identical requests bypass the upstream LLM entirely, saving **100% of input & output tokens**.
2. **Semantic Cache (Vector Search with Cosine Similarity):**
   - **Algorithm:** Leverages prompt embeddings mapped into a **Qdrant** collection, searched via **Cosine Similarity** vector matching.
   - **How it works:** If a new query matches a previously cached prompt with a similarity score exceeding the threshold (e.g. `0.92`), the cached response is returned immediately. This detects similar intents phrased differently, saving **100% of downstream LLM costs**.

---

## 🧠 Knowledge Graph Memory & RAG

Pylos incorporates built-in plugins for dynamic context retrieval and long-term agent memory:

1. **Cross-Agent Memory Graph (Memgraph):**
   - **Technology:** Connects to **Memgraph** utilizing the **Bolt** protocol (`neo4rs` crate) and **Cypher** queries.
   - **How it works:** 
     - **Pre-hook:** Intercepts incoming messages, queries Memgraph for entities and relations linked to the active `VirtualKey`, and injects them as System context.
     - **Post-hook:** Searches model outputs for `<memory>EntityA|RELATION|EntityB</memory>` tags, parses new facts, and merges (`MERGE`) them into the database graph.
2. **Retrieval-Augmented Generation (RAG):**
   - **Technology:** Integrates with **Qdrant** for vector search and retrieves similar entries via embeddings.
   - **How it works:** When targeting model endpoints such as `graphon-rag-emails` or `mnemosyne-search`, Pylos first embeds the user query, searches Qdrant collections (e.g. emails or file logs), builds an augmented context prompt, and forwards the augmented request to the target LLM.

---

## 🛡️ Authentication & Access Control

Pylos supports dual authentication schemes for administration:
1. **Google OAuth (SSO):** Authenticate with corporate accounts. The first user to log in is bootstrapped as the `admin`. Subsequent new sign-ins are registered as `member`s, which can then be assigned to Teams and Organizations.
2. **Admin Key Fallback:** If Google OAuth is not configured or fails, the static `PYLOS_ADMIN_KEY` environment variable can be used to log in.

---

## 📊 API Reference

### Inference Endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/v1/chat/completions` | Unary & Streaming Chat completions |
| `POST` | `/v1/embeddings` | Text embeddings generation |
| `POST` | `/v1/images/generations` | Image generation |
| `GET` | `/v1/models` | List all catalog & dynamic models |

### Management Endpoints (Requires Auth)

| Method | Path | Description |
|---|---|---|
| `GET/POST` | `/providers` | Register and manage upstream LLM providers |
| `GET/POST` | `/virtual-keys` | Manage virtual keys, rate limits, and budgets |
| `GET` | `/api/logs/stats` | View aggregated dashboard usage metrics |
| `POST` | `/config/reload` | Hot-reload configurations |

---

## 🛠️ Development commands

```bash
make test         # Run all unit & integration tests
make lint         # Run clippy checks
make audit        # Run security vulnerability audit on dependencies
```

## 📄 License

TBD
