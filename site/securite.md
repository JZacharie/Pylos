---
layout: default
title: Sécurité & Exploitation
description: Modèle d'authentification, gestion des secrets, exposition réseau, protection des données et checklist de durcissement de Pylos.
permalink: /securite/
---

# Sécurité & Exploitation

Cette page décrit le **modèle de sécurité effectif** de Pylos : qui peut faire quoi, où sont stockés les secrets, ce qui est exposé sur le réseau et comment durcir une installation.

---

## 🔐 1. Trois identités, trois périmètres

Pylos distingue strictement le **plan de contrôle** (administration) du **plan de données** (inférence).

| Identité | Usage | Peut accéder à |
| :--- | :--- | :--- |
| **Clé admin statique** (`PYLOS_ADMIN_KEY`) | CI/CD, scripts, secours | Tout le plan de contrôle (management) |
| **Session JWT** (OAuth Google) | Console d'administration | Plan de contrôle **si et seulement si** le rôle est `admin` |
| **Virtual Key** (`sk-pylos-*`) | Applications et agents | **Uniquement** les routes d'inférence et le proxy MCP |

> ⚠️ **Une Virtual Key n'ouvre jamais l'API de management** : `/providers`, `/virtual-keys`, `/config`, `/api/users`, `/api/logs` et `/virtual-keys/{id}/value` sont réservés à l'administration.

### Vérifications appliquées
- Le rôle administrateur est **relu depuis la base** à chaque requête : un JWT forgé avec `role=admin` sur un compte `member` est rejeté, tout comme un administrateur désactivé.
- Les clés sont comparées en **temps constant** (pas de fuite par timing).
- Le premier utilisateur OAuth **n'est plus** promu administrateur automatiquement (voir §3).

---

## 🚀 2. Initialisation (premier démarrage)

Tant que le mot de passe administrateur n'est pas défini, **toute l'API de management est bloquée** (`403 setup_required`).

1. Ouvrir la console : `http://localhost:8080/setup`
2. Saisir deux fois un mot de passe de **10 caractères minimum**
3. Le hash est stocké en base (`server_secrets`)

Le mot de passe est haché en **Argon2id** avec un sel aléatoire (chaîne PHC `$argon2id$...`). Les installations antérieures dont le hash est encore en **SHA-256 non salé** continuent de fonctionner : la compatibilité est assurée et le secret est ré-haché au prochain `setup`.

---

## 🌐 3. OAuth Google

L'activation nécessite `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` et, de préférence, `GOOGLE_REDIRECT_URI` **imposé côté serveur** (la valeur envoyée par le client n'est utilisée qu'en dernier recours, pour le développement local).

### Protection CSRF (`state`)
1. `GET /api/auth/google/start` génère un **state aléatoire** et le dépose dans un cookie `HttpOnly`, `SameSite=Lax`, 10 minutes.
2. Le client renvoie ce state à Google, qui le restitue au callback.
3. `POST /api/auth/google/callback` **refuse** toute requête sans state, ou dont le state ne correspond pas au cookie (`403`), puis invalide le cookie.

### Attribution du rôle administrateur
Les administrateurs sont déclarés **explicitement** via la variable `ADMIN_EMAILS` (emails séparés par des virgules, insensibles à la casse) :

```bash
ADMIN_EMAILS="cto@exemple.com,platform-owner@exemple.com"
```

- Un email listé devient `admin` (y compris s'il existait déjà en `member`).
- Les autres utilisateurs sont créés en `member` avec le groupe `playgroup`, et doivent être approuvés par un administrateur.

> Sur une instance neuve sans `ADMIN_EMAILS`, **personne** n'est administrateur par OAuth : définissez la variable, ou utilisez `PYLOS_ADMIN_KEY`, avant d'ouvrir la console.

---

## 🔑 4. Secrets et chiffrement

### Clé de chiffrement Pylos
```bash
# 32 octets encodés en base64
PYLOS_ENCRYPTION_KEY="$(openssl rand -base64 32)"
```

| Comportement | Détail |
| :--- | :--- |
| Clé définie | Chiffrement **AES-256-GCM** (nonce aléatoire) des valeurs de Virtual Keys et des secrets providers stockés en base |
| Clé absente, développement | Une clé **éphémère** est générée : les valeurs chiffrées deviennent illisibles après redémarrage (avertissement au démarrage) |
| Clé absente, production | Le serveur **refuse de démarrer** si `PYLOS_ENV=production` ou `PYLOS_STRICT_ENCRYPTION=1` |

**La clé n'est jamais écrite dans les logs.** Le chiffrement utilise un chiffrement **unique du processus**, partagé par tous les threads.

### Ce qui est chiffré au repos
- Les valeurs des Virtual Keys (jamais stockées en clair, seul un SHA-256 permet la recherche).
- Les clés d'API des providers dans la table `gateway_config` (préfixe `enc:v1:`), y compris les identifiants AWS Bedrock.
- Le fichier `pylos.json` reste en clair (il est édité par l'opérateur et protégé par les permissions du système de fichiers) — utilisez `"env.VAR"` pour ne rien écrire du tout.

### Rotation
1. Changer `PYLOS_ENCRYPTION_KEY` **puis** re-saisir les valeurs concernées (elles ne sont pas déchiffrables avec la nouvelle clé).
2. Les Virtual Keys peuvent être régénérées depuis la console ; les clés providers re-saisies via `/providers`.

---

## 🌍 5. Exposition réseau

| Endpoint | Protection |
| :--- | :--- |
| `/v1/chat/completions`, `/v1/embeddings`, `/v1/images/generations` | Virtual Key obligatoire (`Authorization: Bearer sk-pylos-*`) |
| `/mcp/{server}/{path}` (proxy) | Virtual Key ou `X-Team-Id` selon la configuration du serveur MCP |
| `/mcp/{server}` (statut) | **Administrateur uniquement** (expose la topologie interne) |
| `/metrics` | Libre par défaut ; exige un jeton si `PYLOS_METRICS_TOKEN` est défini |
| `/health` | Public (sonde de vivacité) |

### Protection de `/metrics`
```bash
PYLOS_METRICS_TOKEN="un-jeton-long-et-aleatoire"
# puis
curl -H "Authorization: Bearer $PYLOS_METRICS_TOKEN" http://localhost:3000/metrics
```
La comparaison du jeton est faite en temps constant. Définissez cette variable si Prometheus scrape Pylos à travers un réseau non maîtrisé.

### Proxy MCP
Le proxy **ne transmet jamais** au serveur MCP amont : `Authorization`, `X-Admin-Key`, `X-Team-Id`, cookies, en-têtes de proxy (`X-Forwarded-*`, `X-Real-IP`, `Connection`…). Il ne renvoie pas non plus les `Set-Cookie` de l'amont. Le serveur MCP ne peut donc pas récupérer la clé de l'appelant.

---

## 🧭 6. Gouvernance appliquée

| Contrôle | Comportement |
| :--- | :--- |
| **Plafond budgétaire** | Estimation du coût **avant** l'appel ; au dépassement : **HTTP 402** `budget_exceeded` |
| **Quota de requêtes (RPM) / de tokens (TPM)** | Appliqué par le `RateLimitPlugin` sur un **store persistant** : les quotas sont partagés entre répliques et conservés au redémarrage |
| **Expiration de clé** (`expires_at`) | Vérifiée à chaque requête ; une clé expirée est retirée du registre et refusée (**HTTP 401** `key_expired`) |
| **Filtrage des modèles/providers** | Une Virtual Key ne peut utiliser que les modèles/providers de sa configuration |
| **En-têtes de forçage** | `x-pylos-force-provider` / `x-pylos-force-model` **ne peuvent jamais élargir** les droits d'une Virtual Key (401 hors périmètre) |

> Les erreurs de gouvernance (budget, quota, autorisation) interrompent la requête : elles ne sont **jamais ignorées** en cours de pipeline.

---

## 🛡️ 7. Protection des données

- **Garde-fous** : masquage des emails, IBAN, SSN, IP, téléphones, cartes bancaires, clés d'API, JWT et clés privées avant l'envoi au fournisseur.
- **Aucun contournement client** : il n'existe plus d'en-tête permettant de désactiver les garde-fous ; seul l'administrateur peut désactiver le plugin.
- **Journalisation** : le message **original** n'est jamais journalisé ; la correspondance entre placeholders et valeurs réelles (`pii_mapping`) **n'est pas persistée**, et l'aperçu stocké est la version masquée.
- **Cache** : les entrées de cache (préfixe et sémantique) sont **cloisonnées par locataire et par modèle** ; deux Virtual Keys ne partagent jamais de réponse, même avec un prompt identique.
- **Rétention** : `log_retention_days` purge automatiquement les historiques.

---

## ✅ 8. Checklist de durcissement

- [ ] `PYLOS_ENCRYPTION_KEY` définie, **stable** et sauvegardée (32 octets base64)
- [ ] `PYLOS_ADMIN_KEY` long et aléatoire (ou setup via la console), jamais commité
- [ ] `ADMIN_EMAILS` défini **avant** d'activer OAuth Google
- [ ] `GOOGLE_REDIRECT_URI` fixé côté serveur
- [ ] `allowed_origins` restreint (jamais `"*"` en production)
- [ ] `PYLOS_METRICS_TOKEN` défini si /metrics est exposé
- [ ] PostgreSQL configuré dès que plusieurs répliques sont déployées
- [ ] Volume de données chiffré au repos (disque/stockage) et sauvegardes protégées
- [ ] Révocation des jetons d'accès (PAT GitHub, clés providers) stockés dans votre coffre, pas dans un dépôt

---

## 📋 9. Variables d'environnement de sécurité

| Variable | Rôle | Défaut |
| :--- | :--- | :--- |
| `PYLOS_ADMIN_KEY` | Clé d'administration statique | — (setup console) |
| `PYLOS_ENCRYPTION_KEY` | Clé AES-256 base64 (32 octets) | clé éphémère (refusée en production) |
| `PYLOS_STRICT_ENCRYPTION` | Interdit la clé éphémère (`1`/`true`) | `false` |
| `PYLOS_ENV` | `production` active le mode strict | — |
| `ADMIN_EMAILS` | Emails administrateurs (OAuth) | — |
| `PYLOS_METRICS_TOKEN` | Jeton Bearer exigé sur `/metrics` | — (accès libre) |
| `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` | OAuth Google | — |
| `GOOGLE_REDIRECT_URI` | Callback imposé par le serveur | — |
| `JWT_SECRET` | Signature des sessions ; aléatoire par démarrage si absent | aléatoire |

---

*Cette page reflète le comportement du code de la branche `main`. En cas d'écart, le code fait foi.*
