---
layout: default
title: Passerelle LLM & Proxy MCP d'Entreprise
description: Pylos est une passerelle IA haute performance et ultra-faible latence (<2ms) écrite en Rust, unifiant OpenAI, Anthropic, AWS Bedrock, avec gouvernance stricte et cache vectoriel.
permalink: /
---

<div class="hero">
  <div class="hero-badge">
    <span class="dot"></span> Rust Powered &bull; Latence &lt; 2ms &bull; Open Source
  </div>
  <h1 class="hero-title">La Passerelle IA & Proxy MCP pour l'Entreprise</h1>
  <p class="hero-sub">
    Pylos unifie plus de 20 fournisseurs de modèles de langage (OpenAI, Anthropic, Bedrock, Gemini, DeepSeek...) derrière un endpoint unique compatible OpenAI, avec gouvernance budgétaire, cache sémantique vectoriel et mémoire cross-agent.
  </p>
  <div class="hero-cta">
    <a href="{{ '/demarrage/' | relative_url }}" class="btn-primary">⚡ Démarrage Rapide</a>
    <a href="{{ '/architecture/' | relative_url }}" class="btn-secondary">Architecture & Design</a>
    <a href="https://github.com/JZacharie/Pylos" class="btn-secondary" target="_blank">Dépôt GitHub ↗</a>
  </div>
</div>

<div class="kpi-grid">
  <div class="kpi-card">
    <div class="kpi-value">&lt; 2 ms</div>
    <div class="kpi-label">Surcharge de latence moyenne</div>
  </div>
  <div class="kpi-card">
    <div class="kpi-value">20+</div>
    <div class="kpi-label">Fournisseurs LLM unifiés</div>
  </div>
  <div class="kpi-card">
    <div class="kpi-value">100%</div>
    <div class="kpi-label">Économie de tokens sur cache hit</div>
  </div>
  <div class="kpi-card">
    <div class="kpi-value">0 s</div>
    <div class="kpi-label">Temps d'arrêt (Rechargement à chaud)</div>
  </div>
</div>

<figure class="img-frame">
  <img src="{{ '/assets/images/ui-preview.png' | relative_url }}" alt="Console d'Administration Pylos" loading="lazy" />
  <figcaption>Console d'Administration Pylos (React 19 & Vite 8) — Gestion des clés virtuelles, métriques temps réel et gouvernance budgétaire.</figcaption>
</figure>

---

## 🎯 Proposition de Valeur

<div class="grid-3">
  <div class="card">
    <div class="card-icon">⚡</div>
    <h3>Performances Brutes (Rust)</h3>
    <p>Conçu avec Axum et Tokio sur une architecture hexagonale asynchrone, Pylos traite des milliers de requêtes/s avec une consommation mémoire minimale.</p>
  </div>
  <div class="card">
    <div class="card-icon">💰</div>
    <h3>Contrôle Budgétaire & Coûts</h3>
    <p>Attribuez des quotas hebdomadaires/mensuels stricts en USD à chaque clé virtuelle (<code>sk-pylos-*</code>). Arrêt automatique avant dépassement.</p>
  </div>
  <div class="card">
    <div class="card-icon">🛡️</div>
    <h3>Sécurité & Masquage PII</h3>
    <p>Garde-fous intégrés pour filtrer et anonymiser les données sensibles ou personnelles (PII) avant transmission aux fournisseurs d'API.</p>
  </div>
  <div class="card">
    <div class="card-icon">🧠</div>
    <h3>Cache TinyLFU & Qdrant</h3>
    <p>Double niveau de cache : cache de préfixe ultra-rapide (TinyLFU) et cache sémantique vectoriel par similarité cosinus via Qdrant.</p>
  </div>
  <div class="card">
    <div class="card-icon">🔌</div>
    <h3>Proxy MCP Dynamique</h3>
    <p>Permet à vos agents et modèles d'interagir nativement avec des serveurs Model Context Protocol (MCP) en toute isolation.</p>
  </div>
  <div class="card">
    <div class="card-icon">🕸️</div>
    <h3>Graphe de Mémoire Memgraph</h3>
    <p>Persistance et échange de contexte inter-agents via des requêtes Cypher connectées au protocole Bolt de Memgraph.</p>
  </div>
</div>

---

## 🚀 Remplacement Transparent des SDKs OpenAI

Aucune réécriture de code applicatif n'est nécessaire. Changez simplement l'`api_base` (ou `base_url`) de votre client OpenAI vers Pylos et utilisez une clé virtuelle sécurisée :

```bash
curl -X POST http://localhost:3000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-pylos-marketing-team-key" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "user", "content": "Résume les bénéfices de la passerelle Pylos en 3 points."}
    ],
    "temperature": 0.7
  }'
```

Le proxy analyse la clé `sk-pylos-*`, valide les quotas et le débit de requêtes, vérifie les caches (TinyLFU puis Qdrant), applique le masquage des données sensibles, route intelligemment vers le fournisseur optimal avec basculement automatique (*fallback*), et consigne les métriques Prometheus et les logs d'audit.

---

## 📚 Explorer la Documentation

* **[Guide de Démarrage Rapide](demarrage.md)** : Installation locale, Makefile et Docker Compose.
* **[Architecture Système](architecture.md)** : Plongée au cœur de la Clean Architecture hexagonale en Rust.
* **[Configuration `pylos.json`](configuration.md)** : Guide exhaustif de chaque paramètre du fichier de configuration.
* **[Caching & Graphe de Mémoire](caching-memoire.md)** : Détails sur l'éviction TinyLFU, Qdrant et Memgraph.
* **[Proxy MCP & Écosystème Plugins](mcp-plugins.md)** : Extension des capacités des agents et serveurs d'outils.
* **[Gouvernance & Sécurité](gouvernance.md)** : Clés virtuelles, contrôle budgétaire et isolation multi-tenant.
* **[Référence Complète de l'API](api.md)** : Spécifications des endpoints d'inférence et d'administration.
