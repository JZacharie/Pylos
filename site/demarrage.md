---
layout: default
title: Démarrage Rapide
description: Guide d'installation et de mise en route de Pylos en développement local et avec Docker Compose.
permalink: /demarrage/
---

# Guide de Démarrage Rapide

Ce guide vous accompagne pas à pas pour déployer Pylos en quelques minutes, configurer vos premières clés d'API et tester vos appels d'inférence.

---

## 📋 Prérequis

* **Rust & Cargo** : version `1.80+` (pour la compilation native)
* **Docker & Docker Compose** : recommandé pour lancer l'environnement complet (serveur, UI, bases vectorielles et monitoring)
* **Make** : pour exécuter les commandes simplifiées du projet

---

## 🛠️ Option 1 : Déploiement Complet avec Docker Compose (Recommandé)

C'est la méthode la plus rapide pour tester l'ensemble des composants de Pylos (Passerelle Rust, Console Web React, bases SQLite / PostgreSQL, Prometheus et Grafana).

### 1. Cloner le dépôt et initialiser l'environnement

```bash
git clone https://github.com/JZacharie/Pylos.git
cd Pylos

# Créer votre fichier d'environnement
cp .env.example .env
```

### 2. Configurer vos clés API dans `.env`

Ouvrez `.env` et renseignez au moins une clé de fournisseur LLM et votre clé d'administration :

```env
# Clé d'administration statique (pour la console Web)
PYLOS_ADMIN_KEY=sk-pylos-admin-secret-key-12345

# Fournisseurs LLM
OPENAI_API_KEY=sk-proj-...
ANTHROPIC_API_KEY=sk-ant-...

# Port d'écoute du serveur Pylos
PYLOS_PORT=3000
```

### 3. Démarrer la stack

```bash
docker compose up -d
```

Les services sont immédiatement accessibles :
* **Passerelle d'inférence IA** : `http://localhost:3000`
* **Console d'Administration Web** : `http://localhost:8080`
* **Métriques Prometheus** : `http://localhost:9090`
* **Tableaux de bord Grafana** : `http://localhost:3001`

---

## 💻 Option 2 : Exécution Locale en Développement (Rust & Node.js)

Si vous souhaitez contribuer ou compiler Pylos directement sur votre machine hôte :

### 1. Installer les dépendances

```bash
make setup
```

Cette commande installe les dépendances Rust, vérifie la présence de `sqlx-cli` et prépare les dépendances de l'interface `ui/`.

### 2. Lancer le serveur backend et l'interface UI

```bash
make run
```

Le serveur backend Axum démarre avec rechargement automatique si configuré, et l'interface de développement Vite démarre sur le port `5173`.

---

## 🧪 Tester votre Premier Appel d'Inférence

Envoyez une requête de complétion de chat standard au format OpenAI :

```bash
curl -X POST http://localhost:3000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-pylos-default" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {
        "role": "system",
        "content": "Tu es un assistant concis et précis."
      },
      {
        "role": "user",
        "content": "Explique ce quest une passerelle IA en une phrase."
      }
    ],
    "stream": false
  }'
```

### Réponse attendue :

```json
{
  "id": "chatcmpl-pylos-a1b2c3d4",
  "object": "chat.completion",
  "created": 1728628800,
  "model": "gpt-4o-mini",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Une passerelle IA est un point d'accès unifié et sécurisé qui orchestre, optimise et gouverne l'ensemble des flux de requêtes envoyés aux modèles de langage."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 28,
    "completion_tokens": 32,
    "total_tokens": 60
  }
}
```

---

## 🔄 Rechargement à Chaud de la Configuration

Pylos supporte la mise à jour à chaud de son fichier `pylos.json` sans coupure de service ni perte de connexions actives :

```bash
curl -X POST http://localhost:3000/config/reload \
  -H "Authorization: Bearer sk-pylos-admin-secret-key-12345"
```

Le serveur recharge dynamiquement les définitions de modèles, les quotas de clés virtuelles, les pools de connexions et les plugins actifs.

---

## ☸️ Déploiement Kubernetes avec Helm

Pylos inclut des Helm Charts prêts pour la production dans le répertoire `helm/` :

```bash
# Vérifier la validité du chart
helm lint helm/pylos

# Installer le chart sur votre cluster
helm upgrade --install pylos ./helm/pylos \
  --namespace ai-gateway \
  --create-namespace \
  --set replicaCount=3 \
  --set env.PYLOS_ADMIN_KEY="votre-cle-securisee"
```

Consultez le fichier [helm/values.yaml](https://github.com/JZacharie/Pylos/blob/main/helm/pylos/values.yaml) pour personnaliser les ingress, secrets et auto-scaling horizontal (HPA).
