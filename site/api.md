---
layout: default
title: Référence API
description: Spécification technique des endpoints d'inférence OpenAI-compatibles et des routes d'administration de Pylos.
---

# Spécification Technique de l'API

Pylos expose deux familles d'endpoints :
1. **API d'Inférence IA** : strictement compatible avec les spécifications de l'API OpenAI.
2. **API d'Administration** : pour la supervision, la télémétrie et la gestion dynamique des clés et fournisseurs.

---

## ⚡ Endpoints d'Inférence (Compatibles OpenAI)

### 1. `POST /v1/chat/completions`
Crée une complétion de chat à partir d'un historique de messages. Supporte les modes unaire (JSON unique) et streaming (Server-Sent Events).

#### En-têtes :
* `Content-Type: application/json`
* `Authorization: Bearer sk-pylos-...`

#### Corps de la requête :
```json
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "Tu es un assistant utile." },
    { "role": "user", "content": "Quels sont les avantages de Rust pour une passerelle réseau ?" }
  ],
  "temperature": 0.7,
  "max_tokens": 500,
  "stream": false
}
```

---

### 2. `POST /v1/embeddings`
Génère un vecteur d'embedding pour une ou plusieurs chaînes de caractères.

```json
{
  "model": "text-embedding-3-small",
  "input": "Pylos est une passerelle IA performante."
}
```

---

### 3. `GET /v1/models`
Retourne la liste des modèles actifs autorisés pour la clé virtuelle appelante.

```bash
curl -X GET http://localhost:3000/v1/models \
  -H "Authorization: Bearer sk-pylos-default"
```

---

## 🛠️ Endpoints d'Administration

Ces endpoints nécessitent soit une clé d'administration (`Authorization: Bearer $PYLOS_ADMIN_KEY`), soit un jeton de session JWT avec le rôle `admin`.

### 1. `POST /config/reload`
Déclenche la relecture à chaud du fichier `pylos.json`.

```bash
curl -X POST http://localhost:3000/config/reload \
  -H "Authorization: Bearer $PYLOS_ADMIN_KEY"
```

#### Réponse :
```json
{
  "status": "success",
  "message": "Configuration reloaded successfully",
  "timestamp": "2026-10-11T07:30:00Z"
}
```

---

### 2. `GET /api/logs/stats`
Retourne les statistiques consolidées de requêtes, coûts et taux de cache.

#### Réponse :
```json
{
  "total_requests": 142500,
  "cache_hits_prefix": 38200,
  "cache_hits_semantic": 12400,
  "total_tokens_saved": 48920000,
  "active_virtual_keys": 18,
  "p99_latency_ms": 2.1
}
```

---

### 3. `GET /health`
Sonde de disponibilité (Readiness & Liveness Probe pour Kubernetes) :

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_secs": 84210
}
```

---

### 4. `GET /metrics`
Expose les métriques formatées au standard Prometheus pour le scraping Grafana :

```prometheus
# HELP pylos_requests_total Nombre total de requêtes traitées
# TYPE pylos_requests_total counter
pylos_requests_total{provider="openai",model="gpt-4o",status="200"} 98402
pylos_requests_total{provider="anthropic",model="claude-3-5-sonnet",status="200"} 44098

# HELP pylos_request_duration_seconds Latence de traitement de la passerelle
# TYPE pylos_request_duration_seconds histogram
pylos_request_duration_seconds_bucket{le="0.002"} 112000
pylos_request_duration_seconds_bucket{le="0.005"} 141000
```
