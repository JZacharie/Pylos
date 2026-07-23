# Pylos — Passerelle LLM & Proxy MCP en Rust (Entreprise)

<p align="center">
  <img src="logo.png" alt="Pylos Logo" width="200" />
</p>

<p align="center">
  <a href="README.md"><b>English</b></a> |
  <a href="README.fr.md"><b>Français</b></a> |
  <a href="README.es.md"><b>Español</b></a>
</p>

Pylos est une passerelle IA haute performance et à ultra-faible latence écrite en Rust. Elle sert de proxy unifié et sécurisé pour plus de 20 fournisseurs de LLM, offrant un remplacement transparent pour les SDK compatibles OpenAI. Grâce à une gouvernance intégrée, une gestion rigoureuse des coûts, des garde-fous de confidentialité et un tableau de bord d'administration moderne en React, Pylos aide les équipes à déployer leurs flux de travail IA en toute sécurité.

<p align="center">
  <img src="docs/image.png" alt="Aperçu d'AI Gateway" width="800" />
</p>

> [!WARNING]
> **Statut du projet : Non prêt pour la production (Non-Production Ready)**
> Bien que les tests techniques (tests unitaires et d'intégration technique) soient fonctionnels, les **tests d'intégration système (SIT - System Integration Tests)** n'ont pas encore été réalisés. Pylos est actuellement considéré comme **non prêt pour la production**. Une fois les fonctionnalités de test stabilisées, une campagne de tests complète sera menée pour valider définitivement le projet.

## 📖 Ressources & Documentation

* **[Présentation Pylos AI Gateway (PDF)](Pylos_AI_Gateway.pdf)**
* **[Plan d'architecture Pylos AI Gateway (PDF)](docs/Pylos_AI_Gateway_Blueprint.pdf)**

---

## 🎯 Avantages Clés & Proposition de Valeur

- **⚡ Performances de pointe :** Développé en Rust avec des E/S asynchrones (Axum & Tokio), ajoutant une surcharge minime (`< 2ms`).
- **💰 Contrôle des coûts & Budgets :** Évitez les factures imprévues avec un suivi des jetons en temps réel, des budgets hebdomadaires/mensuels en USD et du contrôle de débit (rate limiting) associés à chaque clé virtuelle.
- **🛡️ Sécurité & Confidentialité de niveau entreprise :** Authentification Google OAuth (avec secours par clé d'administration statique `PYLOS_ADMIN_KEY`) pour sécuriser l'interface d'administration. Côté données, des garde-fous masquent automatiquement les données personnelles (PII) et bloquent les contenus sensibles.
- **🔄 Résilience sans interruption :** Basculement automatique (fallbacks), disjoncteur (circuit breaker) et stratégies de réessai automatique avec recul exponentiel pour assurer la continuité de service de vos applications IA.
- **🌐 Gestion Multi-Tenant :** Organisez vos utilisateurs en Organisations et Équipes, et attribuez des Clés Virtuelles (`sk-pylos-*`) configurées avec des accès restreints par modèle et par fournisseur.
- **🔌 Support dynamique MCP (Model Context Protocol) :** Connectez vos agents aux serveurs d'outils tiers via un proxy MCP dynamique.

---

## ✨ Fonctionnalités clés

- **Passerelle IA Unifiée :** Remplacement transparent des endpoints OpenAI (`/v1/chat/completions`, `/v1/embeddings`, etc.) supportant OpenAI, Anthropic, AWS Bedrock, Google Gemini, DeepSeek, Groq, Ollama, OpenRouter, et plus.
- **Routage Intelligent :** Routage dynamique basé sur les modèles disponibles, répartition de charge pondérée et règles de routage CEL (Common Expression Language).
- **Gouvernance stricte :** Clés virtuelles avec limites RPM/TPM configurables et fenêtres de budget actives.
- **Observabilité intégrée :** Métriques Prometheus (`/metrics`), traçage distribué OpenTelemetry et historique des requêtes (SQLite/Postgres) avec histogrammes d'utilisation des jetons.
- **Mise à jour à chaud :** Rechargement de la configuration des modèles, clés virtuelles et fournisseurs sans redémarrer le serveur (`POST /config/reload`).
- **Interface d'administration moderne :** Tableau de bord React 19 + Vite 8 pour gérer les clés, analyser les logs et configurer les règles de sécurité.
- **Cache & Optimisation des jetons (Tokens) :** Intégration d'un cache de préfixe en mémoire (politique d'éviction **TinyLFU** via `moka`) et d'un cache sémantique (correspondance vectorielle par **similitude cosinus** via **Qdrant**) pour éviter les requêtes redondantes et réduire la consommation de tokens LLM.
- **Graphe de Mémoire Cross-Agent :** Mémoire à long terme basée sur **Memgraph** (via le protocole Neo4j Bolt) utilisant des requêtes **Cypher** pour stocker et restituer dynamiquement un graphe d'entités-relations associé aux Clés Virtuelles.
- **RAG (Génération Augmentée par Récupération) Intégrée :** Injection automatique de contexte à partir de collections vectorielles de documents ou d'emails hébergées sur **Qdrant** avant de déléguer la requête aux LLM sous-jacents.

---

## 🏗️ Architecture

Pylos suit une architecture hexagonale (ports et adaptateurs) :

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

## 🚀 Démarrage Rapide

### 1. Lancement local (Développement)

```bash
# Cloner le dépôt
git clone <repo-url> && cd Pylos

# Installer les outils de développement et dépendances
make setup

# Configurer l'environnement
cp .env.example .env
# Éditer .env avec vos clés d'API (OPENAI_API_KEY, PYLOS_ADMIN_KEY, etc.)

# Lancer le serveur backend et l'interface UI
make run
```

### 2. Configuration (`pylos.json`)

Pylos utilise un fichier de configuration centralisé `pylos.json` pour la gouvernance, les budgets, le routage et les plugins. Pour un guide complet sur toutes les clés de configuration disponibles, la syntaxe de substitution des variables d'environnement et les profils de fournisseurs d'API, consultez le [Guide de configuration de Pylos](docs/pylos-config.md).

### 3. Docker Compose (Stack complète)

Démarrez Pylos, l'interface utilisateur, Prometheus et Grafana en une commande :

```bash
docker compose up -d
```
Accédez à la passerelle à l'adresse `http://localhost:3000` et à l'interface d'administration sur `http://localhost:8080`.

---

## ⚡ Cache & Optimisation des Tokens

Pylos réduit la latence et les coûts liés aux appels LLM grâce à deux stratégies de mise en cache complémentaires :

1. **Cache de Préfixe en Mémoire (Éviction TinyLFU) :**
   - **Algorithme :** Géré par une politique d'éviction de cache haute performance **TinyLFU** via la bibliothèque Rust `moka`.
   - **Fonctionnement :** Enregistre la réponse exacte pour un modèle et un historique de messages donnés. Les requêtes strictement identiques suivantes court-circuitent l'appel LLM, économisant **100% des tokens d'entrée et de sortie**.
2. **Cache Sémantique (Recherche Vectorielle par Similitude Cosinus) :**
   - **Algorithme :** Utilise des représentations vectorielles (embeddings) stockées dans une collection **Qdrant**, interrogées avec une métrique de **similitude cosinus**.
   - **Fonctionnement :** Si une nouvelle requête utilisateur est sémantiquement équivalente à une question déjà enregistrée (au-dessus du seuil de similitude configuré, par ex. `0.92`), la réponse en cache est renvoyée immédiatement. Cela permet de détecter des intentions similaires formulées différemment, économisant **100% des coûts du LLM**.

---

## 🧠 Graphe de Mémoire & RAG

Pylos intègre des plugins pour l'injection dynamique de contexte et la mémoire persistante des agents :

1. **Graphe de Mémoire Cross-Agent (Memgraph) :**
   - **Technologie :** Connexion à la base graphe **Memgraph** via le protocole **Bolt** (bibliothèque `neo4rs`) et requêtes **Cypher**.
   - **Fonctionnement :**
     - **Pre-hook :** Intercepte les requêtes entrantes, recherche dans Memgraph les entités et relations liées à la `VirtualKey` active, et les injecte dans le prompt système.
     - **Post-hook :** Analyse la sortie du modèle pour y trouver des balises `<memory>EntitéA|RELATION|EntitéB</memory>`, extrait les nouveaux faits et les fusionne (`MERGE`) dans le graphe de la base.
2. **RAG (Génération Augmentée par Récupération) :**
   - **Technologie :** Intégration avec **Qdrant** pour la recherche vectorielle et la récupération de documents pertinents.
   - **Fonctionnement :** Pour des modèles spécifiques comme `graphon-rag-emails` ou `mnemosyne-search`, Pylos génère l'embedding de la question utilisateur, recherche dans la collection Qdrant (emails ou fichiers), construit un prompt augmenté avec les meilleurs résultats de recherche, et transmet la requête augmentée au LLM final.

---

## 🛡️ Authentification & Contrôle d'Accès

Pylos supporte deux méthodes d'authentification pour l'administration :
1. **Google OAuth (SSO) :** Connexion via des comptes Google. Le premier utilisateur à se connecter obtient automatiquement le rôle `admin` (bootstrap). Les suivants sont créés avec le rôle `member`, permettant à un administrateur de les assigner ensuite à des Équipes et Organisations.
2. **Secours Clé Admin (Fallback) :** Si Google OAuth n'est pas activé ou indisponible, la clé statique `PYLOS_ADMIN_KEY` configurée dans les variables d'environnement permet d'accéder à la console.

---

## 📊 Références de l'API

### Endpoints d'Inférence

| Méthode | Chemin | Description |
| --- | --- | --- |
| `POST` | `/v1/chat/completions` | Completions de chat unaires ou en streaming |
| `POST` | `/v1/embeddings` | Génération de vecteurs d'embeddings |
| `POST` | `/v1/images/generations` | Génération d'images |
| `GET` | `/v1/models` | Liste des modèles disponibles |

### Endpoints de Gestion (Authentification requise)

| Méthode | Chemin | Description |
| --- | --- | --- |
| `GET/POST` | `/providers` | Enregistrement et gestion des fournisseurs LLM |
| `GET/POST` | `/virtual-keys` | Gestion des clés virtuelles, limites et budgets |
| `GET` | `/api/logs/stats` | Vue globale et métriques du tableau de bord |
| `POST` | `/config/reload` | Rechargement dynamique de la configuration |

---

## 🛠️ Commandes de Développement

```bash
make test         # Exécuter les tests unitaires et d'intégration
make lint         # Exécuter les vérifications de code avec Clippy
make audit        # Analyser les vulnérabilités des dépendances
```

## 📄 Licence

À déterminer.
