# MCP Tool Execution Gateway

> **Statut** : Implémenté (Jalon #5, Epics [#142](https://github.com/JZacharie/Pylos/issues/142) → [#148](https://github.com/JZacharie/Pylos/issues/148)).

Pylos agit comme une **MCP Execution Gateway** : il se connecte à des serveurs MCP configurés (transports `stdio`, `http`, `sse`), découvre leurs outils via JSON-RPC 2.0, les expose au format standard OpenAI `tools`, et exécute automatiquement les `tool_calls` émis par les modèles de langage lors des requêtes d'inférence.

---

## 1. Vue d'ensemble de l'architecture

```
[ Client / OpenWebUI / SDK ]
           │
           │ 1. GET /mcp/tools/list (filtré par Virtual Key)
           ▼
[ Pylos Gateway ]
    │  ├─ McpToolRegistry
    │  │    ├─ Built-in: web_fetch (protection SSRF)
    │  │    └─ MCP Servers connectés (stdio / HTTP / SSE)
    │  └─ Governance (allowed_tools / denied_tools par Virtual Key)
    │
    │ 2. POST /v1/chat/completions (avec tools)
    ▼
[ Agent Loop Orchestrator ]
    │
    ├──> Appel Provider LLM (OpenAI / Anthropic / Gemini / Antigravity / Bedrock...)
    │       └─ LLM répond avec choices[0].message.tool_calls
    │
    ├──> Exécution parallèle des outils (tokio::join / timeouts / sandboxing)
    │       └─ ToolRegistry dispatche vers web_fetch ou McpClient
    │
    ├──> Réinjection des réponses (role: "tool") dans l'historique
    │
    └──> Boucle multi-turn jusqu'à la réponse finale (stop) ou max_iterations
```

---

## 2. Transports MCP supportés

Le modèle `McpServer` supporte trois modes de transport (`McpTransportConfig`) :

1. **Stdio** : lance un sous-processus local (ex: `npx`, `python`, binaire natif) et communique via JSON-RPC 2.0 délimité par des retours chariot sur stdin/stdout.
2. **HTTP** : envoie les requêtes JSON-RPC 2.0 via HTTP POST vers l'URL du serveur MCP avec en-têtes optionnels (authentification bearer/api-key).
3. **SSE** : écoute les événements d'initialisation et exécute les appels via POST.

> Rétro-compatibilité : les serveurs sans `transport_config` explicite utilisent automatiquement leur `target_url` comme transport HTTP.

---

## 3. Endpoints API

### 3.1 Liste des outils (`GET /mcp/tools/list`)
Renvoie les outils disponibles au format standard OpenAI `tools`, filtrés selon la Virtual Key de l'appelant (`Authorization: Bearer sk-pylos-...`).

```json
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "web_fetch",
        "description": "Fetch content from a public web page via HTTP GET with HTML-to-text extraction and SSRF protection.",
        "parameters": { ... }
      }
    }
  ]
}
```

### 3.2 Exécution directe d'un outil (`POST /mcp/tools/call`)
Permet à un client ou à un test d'exécuter directement un outil sans passer par une boucle LLM.

```bash
curl -X POST http://localhost:8000/mcp/tools/call \
  -H "Authorization: Bearer sk-pylos-..." \
  -H "Content-Type: application/json" \
  -d '{
    "name": "web_fetch",
    "arguments": { "url": "https://example.com" }
  }'
```

---

## 4. Outil built-in `web_fetch` et protection SSRF

Pylos embarque un outil natif `web_fetch` ne nécessitant aucun serveur externe.
Il intègre une protection stricte contre le SSRF :
- Schémas autorisés : `http://`, `https://`.
- Résolution DNS préalable avec validation de l'IP :
  - Blocage de la loopback (`127.0.0.0/8`, `::1`).
  - Blocage des réseaux privés (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `fc00::/7`).
  - Blocage des adresses link-local (`169.254.0.0/16`, `fe80::/10`).
  - Blocage CGNAT (`100.64.0.0/10`) et broadcast.
- Limite de taille de réponse : 512 Ko.
- Extraction HTML vers texte brut avec normalisation des espaces.

---

## 5. Gouvernance par Virtual Key

Chaque Virtual Key peut restreindre les outils utilisables :
- `allowed_tools` : liste blanche de noms d'outils autorisés (`None` ou `["*"]` = tous).
- `denied_tools` : liste noire d'outils interdits (prioritaire sur l'autorisation).

Si un client tente d'appeler ou de lister un outil refusé pour sa Virtual Key :
- L'outil est omis de `GET /mcp/tools/list`.
- L'exécution via `POST /mcp/tools/call` ou via l'agent loop renvoie `403 Forbidden` (`Tool '<name>' is not allowed for this virtual key`).

---

## 6. Agent Loop Multi-Turn

Lors d'un appel `POST /v1/chat/completions` contenant des outils (ou avec l'en-tête `x-pylos-agent-mode: true` / `x-pylos-auto-tools: true`) :
1. Le modèle est invoqué avec la requête.
2. Si le modèle répond avec `finish_reason: "tool_calls"`, Pylos exécute les appels d'outils en parallèle via `futures::future::join_all` avec timeout configurable (défaut : 30s).
3. Les résultats sont injectés sous forme de messages `role: "tool"` corrélés par `tool_call_id`.
4. La boucle ré-invoque le modèle jusqu'à l'obtention d'une réponse finale textuelle (`finish_reason: "stop"`).
5. Les tokens consommés sur l'ensemble des itérations sont sommés dans le champ `usage` final.
