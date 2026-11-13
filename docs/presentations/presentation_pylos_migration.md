# Présentation Technique : Modernisation de l'Architecture AI Gateway (Pylos)

Ce document présente la stratégie de migration, l'architecture cible, la méthodologie de développement, les pipelines CI/CD, et la plateforme de destination de **Pylos** — un proxy et gateway d'IA haute performance, réécrit de zéro pour répondre aux exigences modernes de production.

---

## 1. Contexte & Stratégie de Transformation

### L'État Initial (Legacy)
La solution initiale reposait sur des architectures passerelles d'IA legacy (inspirées de Bifrost) qui présentaient les limitations suivantes :
*   **Latence & Performance** : Goulots d'étranglement de performance dus à l'exécution de runtimes interprétés/non optimisés sous forte charge.
*   **Monolithique & Rigide** : Difficulté à ajouter de nouveaux fournisseurs d'API LLM (OpenAI, Anthropic, AWS Bedrock, etc.) ou à adapter les politiques de routage dynamiquement.
*   **Absence de Gouvernance** : Pas de gestion fine des clés virtuelles, des quotas budgétaires, des limites de débit (*rate-limiting*) et de la traçabilité.
*   **Configuration Statique** : Nécessité de redémarrer le service à chaque mise à jour de configuration.

### Le Choix de la Modernisation
Le projet a été entièrement repensé et reconstruit pour devenir un système moderne répondant à 100 % aux besoins opérationnels :
*   **Taux de transformation** : **100% de réécriture du backend en Rust** pour garantir une latence ultra-faible, une empreinte mémoire minimale et une sécurité de thread native.
*   ** découplage UI/API** : Création d'une interface web moderne sous React 19 pour piloter la passerelle en temps réel.

---

## 2. Architecture Target (Cible)

Pylos applique les principes de l'**Architecture Hexagonale (Clean Architecture)** divisée en 4 sous-crates Rust autonomes au sein du workspace Cargo :

```mermaid
graph TD
    subgraph pylos-server [pylos-server (Axum HTTP/WS)]
        Routes[Routes HTTP & SSE]
        Middlewares[Middlewares & Metrics]
    end

    subgraph pylos-application [pylos-application]
        Inference[Inference Orchestrator]
        ConfigStore[ConfigStore Hot-Reload]
    end

    subgraph pylos-core [pylos-core (Domain)]
        Entities[Domain Entities]
        Traits[Traits & Ports]
        Errors[PylosError]
    end

    subgraph pylos-infrastructure [pylos-infrastructure (Adapters)]
        OpenAIAdapter[OpenAI Adapter]
        AnthropicAdapter[Anthropic Adapter]
        BedrockAdapter[AWS Bedrock Adapter]
        Persistence[PostgreSQL & SQLx Persistence]
    end

    pylos-server --> pylos-application
    pylos-application --> pylos-core
    pylos-infrastructure --> pylos-core
    pylos-application --> pylos-infrastructure
```

### Rôle de chaque Crate (Backend Rust)
1.  **`pylos-core` (Domaine)** : Contient les règles métier pures, les entités (schémas OpenAI, structures de requêtes/réponses), la gestion d'erreurs centrale (`PylosError`) et les définitions de ports (`traits` comme `Provider` ou `LlmPlugin`). Garanti sans I/O direct.
2.  **`pylos-infrastructure` (Adaptateurs)** : Implémente l'accès aux services externes (SDK AWS Bedrock avec assume-role STS, API Anthropic/OpenAI) et la persistance (base PostgreSQL `pylos` hébergée sur `pg-prd` gérée via `sqlx`).
3.  **`pylos-application` (Cas d'Utilisation)** : Orchestre les requêtes clients via l'indicateur `InferenceOrchestrator` (smart-routing, fallback multi-providers, retry avec exponential backoff et jitter) et gère le rechargement à chaud des configurations sans coupure (`ConfigStore`).
4.  **`pylos-server` (Couche Web)** : Serveur Axum exposant les routes OpenAI-compatibles (`/v1/chat/completions` avec support streaming SSE), l'API d'administration, les métriques Prometheus, et servant d'intermédiaire pour le protocole MCP (*Model Context Protocol*).

### Frontend (Interface d'Administration)
Situé dans `/ui`, il s'appuie sur une stack de pointe :
*   **React 19 & TypeScript 6** piloté par **Vite 8**.
*   **TailwindCSS 3** pour un design fluide, réactif et épuré.
*   **TanStack Query v5** pour la gestion asynchrone des états et requêtes HTTP.
*   **Recharts 3** pour le rendu des métriques temporelles de consommation.
*   **5 Pages Clés** : Dashboard (KPIs en temps réel), Playground (test direct des modèles connectés), Logs (visualisation détaillée des appels), Providers (configuration des clés API fournisseurs), Virtual Keys (création et édition des jetons d'accès applicatifs).

---

## 3. Outils et Stack Technologique

| Couche | Technologie / Outil | Rôle / Bénéfice |
| :--- | :--- | :--- |
| **Backend** | Rust 2021, Tokio 1.36, Axum 0.7 | Concurrence massive, asynchronisme robuste et sécurité mémoire. |
| **Observabilité** | Prometheus + OpenTelemetry | Suivi précis des performances et logs structurés distribués. |
| **Persistance** | PostgreSQL (Sqlx) | Base de données relationnelle centralisée `pylos` hébergée sur l'instance `pg-prd` pour stocker le catalogue de modèles, les logs, les budgets et le rate-limiting. |
| **Frontend** | React 19, TS 6, Vite 8, TailwindCSS 3 | UI moderne, rapide, fluide et typée de bout en bout. |
| **Conteneurisation**| Docker & Docker Compose | Isolation complète des dépendances et de l'environnement d'exécution. |
| **Déploiement** | Kubernetes, Traefik, ArgoCD | Orchestration cloud-native et déploiement continu GitOps. |

---

## 4. Méthodologie de Développement & Pipeline CI/CD

### Méthodologie de Développement
*   **Monorepo** : Regroupement du code backend (crates Rust) et du code frontend (React) pour une meilleure cohérence des versions et des contrats d'API.
*   **Robustesse Locale** : Utilisation d'un `Makefile` pour standardiser les tâches de développement :
    *   `make setup` : Installation des outils de qualité (clippy, rustfmt, cargo-audit, cargo-deny).
    *   `make all` : Formatage, analyse statique (linter) et exécution des tests unitaires et d'intégration locaux.
*   **Pré-commit Hooks** : Vérifications automatiques avant chaque commit pour bloquer le code non formaté ou contenant des secrets enfouis (*GitLeaks*).

### Pipeline CI/CD (GitHub Actions)
Le workflow de déploiement continu s'articule autour de plusieurs étapes automatisées :

```mermaid
sequenceDiagram
    participant Git as Dépôt GitHub
    participant CI as GitHub Actions
    participant Registry as GHCR.io
    participant Argo as ArgoCD (GitOps jo3)
    participant K8s as Cluster Kubernetes

    Git->>CI: Push sur master / main
    activate CI
    CI->>CI: Analyse Statique (Clippy, Fmt)
    CI->>CI: Tests Unitaires & d'Intégration
    CI->>Registry: Build Multi-Arch (amd64/arm64) & Push des Images Docker
    CI->>Argo: Commit de mise à jour de l'image (Repo GitOps)
    deactivate CI
    activate Argo
    Argo->>K8s: Déploiement Progressif (Canary pylos-dev)
    Argo->>K8s: Validation & Promotion (Production pylos)
    deactivate Argo
```

1.  **Validation Qualité** :
    *   Formatage : `cargo fmt --check`
    *   Linter : `cargo clippy -- -D warnings`
    *   Tests : `cargo test --verbose` (inclut les mocks de streaming et l'intégration d'API).
2.  **Compilation & Packaging** :
    *   Build multi-architecture avec **Docker Buildx** pour supporter nativement les processeurs `amd64` (serveurs standards) et `arm64` (optimisation des coûts et performances d'infrastructure).
    *   Publication automatique des images étiquetées avec le SHA du commit vers **GHCR (GitHub Container Registry)**.
3.  **Déploiement Continu (GitOps)** :
    *   Mise à jour automatique des manifests Kubernetes dans le dépôt GitOps dédié `jo3`.
    *   ArgoCD synchronise et applique le changement sur le cluster.

---

## 5. Plateforme de Destination

La plateforme cible est un cluster **Kubernetes** moderne où le gateway est déployé de manière sécurisée et hautement disponible.

*   **Namespace dédié** : `pylos` pour isoler les ressources réseau et de calcul.
*   **Haute Disponibilité** : Déploiement de 2 replicas pour le backend `pylos-server` et la `pylos-ui` avec des limites de ressources strictes (CPU/Mémoire) garantissant la stabilité de la plateforme.
*   **Sécurité d'exécution & des données** :
    *   Politique `runAsNonRoot: true` et drop de toutes les capabilities (`capabilities: drop: [ALL]`).
    *   Secrets d'API et identifiants de base de données injectés de manière sécurisée via **ExternalSecrets** / Kubernetes Secrets.
    *   **Sécurité des clés (Affichage unique)** : La valeur brute d'une clé d'API virtuelle (`sk-pylos-...`) n'est visible que lors de sa création. Elle est ensuite masquée dans la base et obfusquée dans l'API/UI (ex: `sk-py****`), empêchant toute consultation ou vol ultérieur.
*   **Persistance centralisée** : Utilisation d'une base de données PostgreSQL `pylos` sur l'infrastructure `pg-prd`. Les conteneurs devenant stateless, le déploiement multi-replicas gagne en résilience et simplicité de mise à l'échelle (plus besoin de volumes persistants PVC locaux SQLite).
*   **Configuration de la Base de Données (Hybride GitOps / SecOps)** :
    *   **Déclaration GitOps (ConfigMap)** : La clé `"database_url"` est spécifiée dans le fichier `/app/pylos.json` de la ConfigMap `pylos-config`. Elle peut être configurée de deux manières dans GitOps :
        *   **Via variable d'environnement (Recommandé)** : `"database_url": "env.DATABASE_URL"`. L'application résout la valeur dynamiquement depuis l'environnement.
        *   **Via valeur littérale directe** : `"database_url": "postgresql://app:password@cluster-pg-rw.pg-prd.svc:5432/pylos"`.
    *   **Résolution Sécurisée (SecOps & Vault)** : Pour éviter d'exposer les secrets dans le dépôt Git, un `ExternalSecret` (`pylos-database-secret`) est défini dans le GitOps. Il récupère le mot de passe dans HashiCorp Vault (`databases/prd/cluster-pg-app`) et reconstruit la variable d'environnement `DATABASE_URL` avec le host et le port de la base de données ciblée (`cluster-pg-rw.pg-prd.svc:5432/pylos`).
*   **Séparation des flux de configuration (GitOps vs Manuel/DB)** :

| Type de Donnée / Configuration | Source de Vérité (GitOps) | Source de Vérité (Manuel / DB) | Description & Utilisation |
| :--- | :---: | :---: | :--- |
| **Accès Base de Données** | **Oui** | Non | URL, hôte, port et résolutions de secrets pour PostgreSQL (`DATABASE_URL`). |
| **Topologie & Réseau** | **Oui** | Non | Ports d'écoute HTTP, configurations CORS, hôtes réseaux, base URLs des fournisseurs. |
| **Identifiants Fournisseurs (API Keys)** | **Oui** | Non | Clés d'API tierces (OpenAI, Anthropic, Gemini, Groq, Bedrock) injectées via variables d'environnement (`env.GEMINI_API_KEY`, etc.) résolues par ExternalSecrets. |
| **Clés Virtuelles (Virtual Keys)** | Optionnel (bootstrap) | **Oui (Table `virtual_keys`)** | Clés créées dynamiquement par les administrateurs depuis l'UI pour authentifier les utilisateurs ou applications consommatrices. |
| **Catalogue de Modèles & Tarification** | Non | **Oui (Table `model_catalog`)** | Définition des modèles supportés, de leurs alias et de leurs coûts (prompt, completion, image) modifiables sans redéploiement. |
| **Limites de Débit (Rate Limits)** | Non | **Oui (Table `rate_limits`)** | Règles de limitation de requêtes configurées par l'administrateur. |
| **Quotas & Budgets** | Non | **Oui (Table `budgets`)** | Budgets alloués aux utilisateurs/applications et consommation en temps réel. |
| **Historique & Logs** | Non | **Oui (Table `gateway_logs` / `playground_logs`)** | Traces d'appels d'inférence et historique des tests interactifs effectués sur le Playground. |

*   **Routage & Ingress** : Exposition via **Traefik Ingress Route** avec support HTTPS et configuration de serveurs de transport sécurisés (`serverstransport.yaml`).

