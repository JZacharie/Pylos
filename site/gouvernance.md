---
layout: default
title: Gouvernance & Sécurité
description: Gestion multi-tenant, clés virtuelles, contrôle budgétaire en USD, authentification Google OAuth et audit de sécurité.
permalink: /gouvernance/
---

# Gouvernance & Sécurité d'Entreprise

Pylos fournit aux directeurs techniques et responsables de sécurité (CISO) une suite complète d'outils pour administrer, superviser et restreindre l'utilisation de l'intelligence artificielle au sein de leur organisation.

---

## 🔑 1. Clés Virtuelles (`sk-pylos-*`)

Plutôt que de distribuer vos clés d'API directes (OpenAI, Anthropic) aux développeurs et services, vous générez des **clés virtuelles Pylos** configurables individuellement.

### Caractéristiques d'une Clé Virtuelle :
* **Filtrage des Modèles (Allowlist)** : Restreignez la clé à une liste stricte de modèles autorisés (ex. uniquement `gpt-4o-mini`). En format `version: 2`, une liste vide applique une politique de refus total (*deny-all*).
* **Contrôle de Débit (Rate Limiting)** :
  * **RPM (Requests Per Minute)** : Plafonne le nombre de requêtes par minute pour éviter les boucles infinies de code.
  * **TPM (Tokens Per Minute)** : Plafonne le débit de jetons consommés par minute.
  * **Partagé et persistant** : les compteurs sont conservés dans le store (PostgreSQL recommandé dès qu'il y a plusieurs répliques) et survivent aux redémarrages.
* **Expiration (`expires_at`)** : une clé dont la date d'expiration est dépassée est refusée (`HTTP 401 key_expired`) et retirée du registre.
* **Plafond Budgétaire en USD** : Fixe une limite de dépenses infranchissable sur une période glissante ou calendaire.

---

## 💰 2. Moteur de Budgets & Quotas Financiers

Le moteur de budget de Pylos calcule le coût réel de chaque requête en temps réel en appliquant les grilles tarifaires officielles par millier de tokens d'entrée et de sortie.

```
┌────────────────────────────────────────────────────────┐
│ Requête entrante (Clé sk-pylos-marketing)             │
│ Dépense cumulée ce mois : $98.40 / Plafond : $100.00   │
└───────────────────────────────────┬────────────────────┘
                                    │
                                    ▼
       L'appel estimé dépasserait-il le plafond ?
            │                               │
          [OUI]                           [NON]
            │                               │
            ▼                               ▼
    HTTP 402 Payment Required       Requête autorisée
    {"error": "Budget exceeded"}    Comptabilisation des tokens
```

* **Plafond strict (Hard Cap)** : Dès que le budget est atteint, toute requête ultérieure est immédiatement rejetée avec une erreur explicite, protégeant l'organisation contre toute facture imprévue.
* **Fenêtres de réinitialisation** : Prise en charge des fenêtres journalières, hebdomadaires (`1w`), ou mensuelles (`1M`).

---

## 🛡️ 3. Authentification & Rôles

La console d'administration et les APIs de gestion supportent deux niveaux de contrôle :

### 1. Google OAuth (SSO Entreprise)
* Connexion sécurisée avec vos comptes professionnels Google Workspace.
* **Anti-CSRF** : l'état OAuth est émis par le serveur (`GET /api/auth/google/start`) et vérifié au callback ; aucun état fixe côté client n'est accepté.
* **Administrateurs explicites** : le rôle `admin` est accordé aux emails listés dans la variable `ADMIN_EMAILS`. La promotion automatique du premier inscrit a été **supprimée**.
* Les autres utilisateurs reçoivent le rôle `member` (groupe `playgroup`) et doivent être approuvés par un administrateur.

### 2. Clé Admin Statique (`PYLOS_ADMIN_KEY`)
* Utilisée pour l'automatisation CI/CD, les scripts Terraform/Ansible ou comme secours si le fournisseur OAuth est indisponible.
* Le mot de passe défini via l'écran de setup est haché en **Argon2id** (sel aléatoire, format PHC).

### 3. Périmètre des Virtual Keys
* Les clés `sk-pylos-*` sont strictement limitées au **plan de données** : elles n'ouvrent jamais l'API de management ni la révélation de clés.
* Détail du modèle d'autorisation : voir [Sécurité & Exploitation](/securite/).

---

## 📊 4. Observabilité & Conformité RGPD

* **Métriques Prometheus (`/metrics`)** :
  * Protection optionnelle par jeton : définir `PYLOS_METRICS_TOKEN` pour exiger `Authorization: Bearer <jeton>`.
  * Compteurs de requêtes par modèle, statut HTTP et clé virtuelle.
  * Histogrammes de latence totale et latence premier token (TTFT).
  * Histogrammes d'usage de tokens d'entrée et de sortie.
* **Journalisation Sécurisée & RGPD** :
  * Option `"disable_content_logging": true` pour n'enregistrer que les métadonnées (durée, tokens, modèle) sans conserver le texte des messages sensibles.
  * Politique de rétention paramétrable (`log_retention_days`) avec purge automatique des historiques anciens.
