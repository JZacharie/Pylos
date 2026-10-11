# Revue de code Pylos — 11 octobre 2026

> **Objet** : revue indépendante de sécurité et de qualité du dépôt `Pylos` (API Gateway LLM en Rust).
> **Commit audité** : `9a01264a694096dcf5459b83c79288aaf32c3909` (`main`, 2026-10-09).
> **Date de la revue** : 2026-10-11.
> **Auteur** : agent d'audit (DeepSeek Harness), sur demande de J. Zacharie.
> **Statut du projet** : le README déclare explicitement « Not Production Ready » (SIT non réalisés). Cette revue confirme que ce statut est justifié.

---

## 1. Résumé exécutif

Le socle technique est **de bonne facture** : architecture hexagonale propre (core / application / infrastructure / server), SQL entièrement paramétré, chaîne CI (clippy `-D warnings`, 103 tests, CodeQL, MegaLinter), secrets non commités, valeurs de Virtual Keys chiffrées AES-256-GCM, en-têtes Helm avec `runAsNonRoot`. Les builds et tests passent localement (section 4).

En revanche, la revue met en évidence **des défauts d'autorisation critiques** sur le plan de contrôle (management API) ainsi qu'une série de **contournements des mécanismes de gouvernance** (guardrails, budget, restrictions de provider, expiration des clés). Les affirmations du README (« automatique multi-provider fallbacks », « block unsafe content », « budgets ») ne sont que partiellement vraies dans le code.

### Synthèse par sévérité

| Sévérité | Nombre | Thèmes |
|---|---|---|
| 🔴 Critique | 2 | Escalade de privilèges vers l'API de management (Virtual Key et JWT sans contrôle de rôle) |
| 🟠 Élevée | 7 | Bypass guardrails, fuite de PII/secrets dans les logs, contournement provider/modèle, budget inopérant, fuite d'en-têtes via le proxy MCP, fallback limité aux timeouts, PAT GitHub en clair sur disque |
| 🟡 Moyenne | 11 | Hash admin sans sel, clé de chiffrement éphémère loguée, expiration VK non appliquée, clés providers en clair, OAuth sans `state`, endpoints non authentifiés, guardrails non bloquants, unmasking streaming inopérant, rate-limit mémoire, remapping silencieux de modèle, 211 `unwrap()` |
| 🔵 Faible / Info | 8 | Comparaison non constant-time, clés VK prédictibles, artefacts DB/logs versionnés, `fmt` désactivé, rapports MegaLinter en erreur commités, ports Docker exposés, comptage de tokens streaming approximatif, semantique du disjoncteur |

**Aucun secret d'API n'est présent dans les fichiers versionnés** (scan regex sur l'ensemble du dépôt : 0 correspondance), mais un **PAT GitHub est présent en clair dans `Pylos/.git/config`** (remote `origin`), hors du contenu versionné (voir F-09).

---

## 2. Périmètre et méthodologie

* **Périmètre** : `crates/` (4 crates Rust, ~108 fichiers `.rs`), `ui/src` (React/TypeScript), `helm/`, `docker-compose.yml`, `.github/workflows/`, configuration racine. Exclus : `target/`, `node_modules/`, `ui/dist/`.
* **Méthode** : lecture manuelle ciblée des chemins critiques (authentification, middleware, inférence, gouvernance, chiffrement, persistance), recherche de motifs à risque (`unwrap`/`expect`/`panic!`/`unsafe`/secrets/SQL construit dynamiquement), puis vérification par exécution.
* **Hors périmètre** : test d'intrusion dynamique, revue ligne-à-ligne exhaustive des 70+ fichiers front-end, audit des dépendances tierces (`cargo audit` non exécuté), revue des PDF de documentation.
* **Limites** : l'absence de SIT et d'environnement multi-répliques signifie que certains défauts (F-11, F-18) sont des constats d'implémentation, non reproduits en conditions réelles.

---

## 3. Constats détaillés

### 🔴 F-01 — Escalade de privilèges : toute Virtual Key active ouvre l'API de management

* **Sévérité** : Critique — **CVSS approximatif 9.1** (AV:N/AC:L/PR:L/UI:N/S:C/C:H/I:H/A:H)
* **Fichiers** : `crates/pylos-server/src/middleware/management_auth.rs:104-148`, `crates/pylos-server/src/routes.rs:48-108,242-245`

Le middleware `management_auth_middleware` protège **l'intégralité** des routes de management. Or il accepte comme authentification valide **n'importe quelle Virtual Key `sk-pylos-*` active** (branche n°3, lignes 104-148) :

```rust
// 3. Fallback sur la validation de la Virtual Key si c'est un sk-pylos-*
if provided_key.starts_with(pylos_core::domain::virtual_key::VIRTUAL_KEY_PREFIX) {
    if let Ok(Some(vk_cfg)) = state.vk_store.get_key_by_value(&provided_key).await {
        if vk_cfg.is_active {
            ...
            return next.run(request).await;
```

Or les routes ainsi protégées comprennent (routes.rs:96-112, 139-143) :
* `POST/PUT/DELETE /virtual-keys` → création de clés arbitraires ;
* `GET /virtual-keys/{id}/value` → **révélation en clair de la valeur de n'importe quelle clé** (`config.rs:582-603`) ;
* `GET/POST/PUT /providers`, `POST /providers/{name}/test` → modification des clés d'API des providers ;
* `POST /config/reload`, `GET /config` ;
* `/api/users`, `/api/organizations`, `/api/access-groups`, `/api/policies`, `/api/tool-policies` → création/suppression d'utilisateurs (dont admin) ;
* `/api/logs`, `/api/guardrails/events` → accès aux logs de tous les tenants.

**Impact** : un simple consommateur disposant d'une Virtual Key de data-plane (par définition distribuée aux applications) peut se comporter en administrateur : exfiltrer les clés des autres locataires, modifier la configuration des providers, créer des comptes, ou déclencher un `/config/reload` malveillant. C'est une rupture complète du modèle multi-tenant revendiqué par le README.

**Recommandation** : supprimer la branche Virtual Key du middleware de management (ou la restreindre à un ensemble de routes explicitement en lecture seule et scopées au propriétaire de la clé). Le middleware doit exiger soit la clé admin (`PYLOS_ADMIN_KEY`), soit un JWT de session avec `role == "admin"`.

---

### 🔴 F-02 — Aucun contrôle de rôle sur les JWT de session pour l'API de management

* **Sévérité** : Critique — **CVSS approximatif 8.8**
* **Fichiers** : `crates/pylos-server/src/middleware/management_auth.rs:78-94`, `crates/pylos-server/src/interfaces/http/auth.rs:41-49`

Le middleware décode le JWT de session (`PylosSessionClaims`) puis **vérifie uniquement que l'utilisateur existe et est actif** :

```rust
if let Ok(token_data) = jsonwebtoken::decode::<PylosSessionClaims>(...) {
    let email = token_data.claims.sub.to_lowercase();
    if let Ok(users) = state.org_store.list_users().await {
        if let Some(user) = users.iter().find(|u| u.email.to_lowercase() == email && u.is_active) {
            request.extensions_mut().insert(Some(user.clone()));
            return next.run(request).await;
        }
    }
}
```

Le champ `role` du claim n'est **jamais comparé à `"admin"`**. Un utilisateur `member` (ou `playgroup`, rôle attribué automatiquement aux nouveaux inscrits OAuth, `auth.rs:201-207`) obtient donc les mêmes droits que l'administrateur. À comparer avec `middleware/admin_guard.rs:47-56`, qui effectue bien ce contrôle — l'incohérence entre les deux middlewares est manifeste.

**Recommandation** : factoriser un unique middleware d'administration qui (a) valide la signature, (b) exige `role == "admin"` **relu depuis le store** (et non depuis le token, pour ne pas dépendre d'un rôle périmé), (c) refuse les Virtual Keys. Ajouter un test d'intégration « member → 403 sur /api/users ».

---

### 🟠 F-03 — Contournement total des guardrails par simple en-tête client

* **Sévérité** : Élevée
* **Fichier** : `crates/pylos-application/src/guardrails.rs:455-462`

```rust
if ctx.headers.get("x-bypass-guardrails").map(|s| s == "true").unwrap_or(false) {
    return Ok(None);   // ← sortie immédiate : aucune modération, aucun masquage
}
```

N'importe quel appelant peut désactiver la modération de contenu et le masquage PII/secrets avec un en-tête HTTP. L'en-tête n'est pas listé dans les en-têtes CORS autorisés (`routes.rs:309-317`), ce qui ne protège que des navigateurs : un client direct (SDK, curl) n'est pas concerné.

**Recommandation** : supprimer ce mécanisme ou le réserver à une clé administrateur explicitement contrôlée.

---

### 🟠 F-04 — PII et secrets écrits en clair dans les journaux applicatifs et la base de logs

* **Sévérité** : Élevée
* **Fichiers** : `crates/pylos-application/src/guardrails.rs:664-667`, `crates/pylos-server/src/interfaces/http/inference.rs:223-245,534-557`

Alors que le masquage existe, deux fuites annulent son bénéfice :

1. La pré-hook guardrails journalise le message **original**, non masqué, au niveau `info` :
   ```rust
   info!("[Guardrails] Original message:\n{}\n[Guardrails] Obfuscated message:\n{}", original, masked);
   ```
2. Le handler d'inférence persiste en base `input_preview` (message utilisateur brut) **et** `pii_mapping`, qui contient la correspondance `[EMAIL_1] → adresse réelle` (`x-pii-mapping`, construit ligne 689-691 de `guardrails.rs`).

Le masquage est donc **purement cosmétique vis-à-vis de l'observabilité** : les données sensibles restent stockées en clair dans `pylos-logs.db`/`pylos-logs` (Postgres), tables que la revue suppose soumises à des exigences RGPD.

**Recommandation** : ne journaliser que le message masqué ; ne jamais persister `pii_mapping` (ou le chiffrer avec une clé dédiée et une rétention courte) ; tronquer/anonymiser `input_preview` quand `mask_pii` est actif.

---

### 🟠 F-05 — Contournement des restrictions de provider/modèle via en-têtes `x-pylos-force-*`

* **Sévérité** : Élevée
* **Fichiers** : `crates/pylos-server/src/interfaces/http/inference.rs:109-117`, `crates/pylos-application/src/use_cases/inference.rs:170-175,212-220,440-441`

Les en-têtes `x-pylos-force-provider` et `x-pylos-force-model` proviennent directement du client (`headers.get(...)`) et sont prioritaires sur les restrictions de la Virtual Key :

```rust
let allowed_providers = if let Some(forced_prov) = &ctx.force_provider {
    providers.iter().filter(|(p, _)| p.name() == forced_prov).collect()   // ignore ctx.provider_configs
} else if ctx.virtual_key.is_some() { /* filtrage par VK */ } else { ... };
```

De même, `force_model` remplace le modèle demandé et n'est pas validé contre `allowed_models`.

**Impact** : un tenant dont la clé est limitée au modèle le moins cher peut forcer un modèle frontier (coût ×50) ou un provider non autorisé, cassant le contrôle budgétaire et la segmentation de coûts.

**Recommandation** : n'autoriser ces en-têtes que pour une clé admin, ou les valider contre `ctx.provider_configs` avant usage.

---

### 🟠 F-06 — L'application des budgets cible l'alias de la clé, pas son identifiant → contrôle silencieusement inopérant

* **Sévérité** : Élevée
* **Fichiers** : `crates/pylos-server/src/interfaces/http/inference.rs:126-129`, `crates/pylos-application/src/budget_store.rs:334-351`, `crates/pylos-server/src/state.rs:783-789`, `crates/pylos-server/src/interfaces/http/auth.rs:281-290`

Le contexte d'inférence est renseigné avec le **nom** (alias lisible) de la clé :
```rust
if let Some(vk) = &vk_info { ctx.virtual_key = Some(vk.name.clone()); ... }
```
alors que les budgets sont enregistrés sous `virtual_key_id` (identifiant, ex. `vk-1`, `vk-<alphanum>`). `BudgetPlugin::pre_hook` passe donc l'alias à `check_budget`, qui, en l'absence de ligne correspondante, **retourne `Ok(())` sans limite** (`budget_store.rs:144-158`). Un alias « Default Key for x@y.z » ne correspondra jamais à l'id `vk-ab12...`.

**Impact** : sauf coïncidence où l'opérateur nomme chaque clé exactement comme son id, **aucun plafond budgétaire n'est appliqué**, alors que le README en fait un argument commercial majeur.

**Recommandation** : transporter `VirtualKeyInfo { id, name, ... }` et alimenter `ctx.virtual_key` avec l'id ; ajouter un test d'intégration « budget dépassé → 402 ».

---

### 🟠 F-07 — Fuite d'identifiants vers les serveurs MCP cibles (proxy)

* **Sévérité** : Élevée
* **Fichier** : `crates/pylos-server/src/mcp_proxy.rs:96-104`

```rust
for (key, value) in headers.iter() {
    let key_str = key.as_str().to_string();
    if let Ok(val_str) = value.to_str() {
        if key_str != "host" {
            proxy_req = proxy_req.header(&key_str, val_str);   // inclut Authorization, X-Admin-Key, X-Team-Id
        }
    }
}
```

Le proxy transmet **tous** les en-têtes du client au serveur MCP amont, y compris `Authorization: Bearer sk-pylos-...`. Un serveur MCP tiers (potentiellement compromis) récupère ainsi la clé virtuelle de l'appelant. Les en-têtes de réponse sont également recopiés sans filtrage (lignes 121-134).

**Recommandation** : liste blanche d'en-têtes (Content-Type, Accept, User-Agent…), suppression systématique de `Authorization`, `X-Admin-Key`, `X-Team-Id` et des cookies.

---

### 🟠 F-08 — Le fallback multi-provider ne fonctionne que sur timeout

* **Sévérité** : Élevée (écart fonctionnalité/documentation)
* **Fichiers** : `crates/pylos-application/src/use_cases/inference.rs:520-534` et `646-660`, `crates/pylos-core/src/error.rs:42-49`

Après épuisement des retries d'un provider, seules les erreurs `Timeout` passent au provider suivant :

```rust
Err(e) => {
    self.record_failure(provider.name());
    ...
    if !matches!(e, PylosError::Timeout(_)) {
        return Err(e);       // ← erreur 500/429 provider : abandon immédiat du fallback
    }
    last_error = Some(e);
}
```

Or `is_retriable()` considère `ProviderError` et `RateLimitExceeded` comme réessayables. Conséquence : si le premier provider renvoie un 5xx ou un 429 persistant, la requête échoue **même si un provider sain est configuré** — contrairement à la promesse « Automatic multi-provider fallbacks » et « Zero-Downtime Resilience » du README.

**Recommandation** : déclencher le fallback pour toutes les erreurs de `is_retriable()` (et `AllProvidersFailed` seulement en fin de chaîne).

---

### 🟠 F-09 — PAT GitHub en clair dans la configuration git locale

* **Sévérité** : Élevée (hygiène de secrets, hors contenu versionné)
* **Fichier** : `Pylos/.git/config` (remote `origin`)

L'URL du remote embarque un jeton personnel GitHub (`https://ghp_…@github.com/JZacharie/Pylos.git`). Le jeton n'est **pas** dans le contenu versionné, mais il est stocké en clair sur le poste et dans l'historique des commandes éventuelles ; il donne un accès en écriture au dépôt (et aux workflows).

**Recommandation** : révoquer/faire tourner le jeton, nettoyer le remote (`git remote set-url origin https://github.com/JZacharie/Pylos.git`) et utiliser un gestionnaire d'identifiants (Credential Manager / `gh auth`). Vérifier également qu'aucune copie n'existe dans d'autres dépôts du poste.

---

### 🟡 F-10 — Mot de passe administrateur haché en SHA-256 non salé

* **Sévérité** : Moyenne
* **Fichiers** : `crates/pylos-server/src/state.rs:874-879`, `crates/pylos-server/src/routes/setup.rs:80`, `crates/pylos-application/src/config_store.rs:499-545`

Le hash stocké dans `server_secrets` est un simple `SHA-256(password)`, sans sel ni fonction de dérivation (Argon2id, scrypt, bcrypt). En cas de fuite de la base (fichier SQLite copié, sauvegarde, accès Postgres), un attaquant peut casser le mot de passe par dictionnaire à très faible coût.

**Recommandation** : Argon2id (crate `argon2`) avec sel aléatoire et paramètres OWASP, migration transparente de l'ancien hash.

---

### 🟡 F-11 — Clé de chiffrement éphémère, journalisée en clair, et stockée en `thread_local`

* **Sévérité** : Moyenne (perte de données + secret dans les logs)
* **Fichier** : `crates/pylos-core/src/domain/encryption.rs:11-38`

Si `PYLOS_ENCRYPTION_KEY` est absente :
```rust
let b64 = BASE64.encode(key);
tracing::warn!("PYLOS_ENCRYPTION_KEY not set — generated ephemeral key ...: {b64}");
```
La clé AES est donc **écrite dans les logs** puis perdue au redémarrage, rendant indéchiffrables toutes les valeurs de Virtual Keys stockées (`virtual_key_store.rs:129-148` remonte alors « encryption key may have changed »). De plus, le chiffrement passe par une `thread_local!` initialisée paresseusement : chaque thread génère **sa propre clé** quand la variable d'environnement est absente, ce qui rend le déchiffrement inter-thread impossible (le store SQLx multi-thread échouera de façon non déterministe).

**Recommandation** : refuser le démarrage si `PYLOS_ENCRYPTION_KEY` est absente en production (ou la persister dans un trousseau), initialiser le cipher une seule fois dans un `OnceLock` global, et ne jamais journaliser la clé.

---

### 🟡 F-12 — L'expiration des Virtual Keys n'est jamais appliquée

* **Sévérité** : Moyenne
* **Fichiers** : `crates/pylos-server/src/middleware/virtual_key.rs:36-168`, `crates/pylos-core/src/domain/config.rs:584`

Le champ `expires_at` est créé, mis à jour (`config.rs:328,462-463`), persisté en base, mais **aucun chemin de requête ne le compare à l'heure courante** (recherche `expires_at` sur `crates/` : seules des écritures/lectures de persistance). Une clé expirée reste indéfiniment valide.

**Recommandation** : contrôle dans `virtual_key_middleware` (et `management_auth`) avec réponse 401 `key_expired`, plus test associé.

---

### 🟡 F-13 — Clés d'API des providers stockées en clair dans la base de configuration

* **Sévérité** : Moyenne
* **Fichiers** : `crates/pylos-application/src/config_store.rs:552-566,323-329,393-411`

`upsert_provider` insère la `ProviderConfig` complète (y compris `keys[].value`) dans le JSON de la table `gateway_config`/`server_secrets` **sans chiffrement**. Seules les valeurs de Virtual Keys bénéficient d'AES-GCM. Un accès en lecture à la base Postgres/SQLite expose donc les clés OpenAI/Anthropic/AWS/GitHub de production.

**Recommandation** : chiffrer les `EnvVar::Literal` des providers avec la même enveloppe que les Virtual Keys, ou ne persister que des références d'environnement (`env://`).

---

### 🟡 F-14 — OAuth : pas de `state` (CSRF), auto-provisionnement et « premier inscrit = admin »

* **Sévérité** : Moyenne
* **Fichier** : `crates/pylos-server/src/interfaces/http/auth.rs:24-28,55-98,186-294`

1. La requête de callback (`POST /api/auth/google/callback`, `CallbackRequest { code, redirect_uri }`) ne transporte **aucun paramètre `state`** : pas de protection contre le login-CSRF (un attaquant peut faire connecter la victime à une session qu'il contrôle).
2. Tout email Google inconnu est **auto-provisionné** comme utilisateur actif, et **le premier utilisateur de l'installation reçoit le rôle `admin`** (`users.is_empty() → "admin"`). Sur un déploiement neuf où Google OAuth est activé avant toute création d'utilisateur, le premier à se connecter devient administrateur de la passerelle.
3. `redirect_uri` étant fourni par le client, la robustesse repose entièrement sur la configuration Google.

**Recommandation** : implémenter `state` (valeur aléatoire liée à un cookie/session), supprimer l'élévation automatique du premier utilisateur au profit d'une liste d'emails autorisés (`ADMIN_EMAILS`), et durcir le `redirect_uri` (liste blanche serveur uniquement).

---

### 🟡 F-15 — Points d'entrée non authentifiés révélant l'infrastructure

* **Sévérité** : Moyenne
* **Fichiers** : `crates/pylos-server/src/routes.rs:259-265`, `crates/pylos-server/src/mcp_proxy.rs:153-178`

Les routes `/mcp/{server_name}` (statut) et `/mcp/{server_name}/{*path}` sont montées **sans aucun `.layer(...)`** : la vérification est déléguée au handler. Le handler de statut, lui, ne vérifie rien et renvoie `target_url`, le type et le statut du serveur MCP à tout appelant anonyme — information utile à la cartographie interne. `/metrics` (Prometheus) et `/health` sont également publics.

**Recommandation** : appliquer le middleware de management au endpoint de statut, restreindre `/metrics` (auth bearer ou réseau interne), ne pas exposer `target_url`.

---

### 🟡 F-16 — Les guardrails journalisent mais ne bloquent pas

* **Sévérité** : Moyenne (écart documentation/sécurité)
* **Fichiers** : `crates/pylos-application/src/guardrails.rs:604-654`

Les messages sont explicites : `"Keyword match detected (logging but not blocking)"` (l.610) et `"Prompt injection detected (logging but not blocking)"` (l.639). La détection positionne seulement des drapeaux de contexte (`guardrail_triggered`), sans jamais court-circuiter la requête (`pre_hook` peut pourtant retourner `Some(PylosResponse)`). Le README affirme que Pylos « block unsafe content ». Par ailleurs la modération repose sur une liste de regex statiques (`MOD_CATEGORIES`, l.40+) avec un taux élevé de faux positifs (« kill », « bomb ») et de faux négatifs.

**Recommandation** : soit rendre le blocage effectif et configurable (mode `block`/`monitor`), soit corriger la documentation ; documenter l'heuristique et son périmètre.

---

### 🟡 F-17 — Le démasquage PII est inopérant en streaming

* **Sévérité** : Moyenne
* **Fichiers** : `crates/pylos-application/src/use_cases/inference.rs:851-938`, `crates/pylos-server/src/interfaces/http/inference.rs:412-439`

En streaming, `wrap_stream_with_post_hooks` accumule les chunks et reconstruit une `PylosResponse` pour exécuter les post-hooks (dont le démasquage `restore_text`). Mais cette réponse reconstruite est **jetée** : le flux SSE envoyé au client est construit directement à partir des chunks du provider (`sse_stream`, l.412-433). Les placeholders (`[EMAIL_1]`…) éventuellement générés par le modèle ne sont donc jamais restaurés, et tout post-hook correctif (structured output, budget) ne s'applique pas à la réponse visible.

**Recommandation** : appliquer le démasquage côté handler de streaming (transformer les deltas) ou documenter cette limitation et l'assumer.

---

### 🟡 F-18 — Rate limiting RPM en mémoire, incohérent en multi-répliques

* **Sévérité** : Moyenne
* **Fichiers** : `crates/pylos-core/src/domain/virtual_key.rs:93-148`, `crates/pylos-server/src/middleware/virtual_key.rs:135-167`, `crates/pylos-application/src/rate_limit_store.rs`

Le RPM est vérifié et incrémenté dans un `HashMap` **par processus** (`VirtualKeyRegistry`), remis à zéro à chaque redémarrage. Le TPM/RPM du `RateLimitPlugin` utilise au contraire le store persistant. Selon le nombre de répliques, les limites effectives sont donc multipliées ; deux mécanismes distincts coexistent pour la même notion de « rate limit », ce qui complique l'audit et peut diverger.

**Recommandation** : unifier l'application (store persistant ou Redis) et documenter la sémantique de fenêtre (le commentaire « 1 minute glissante » est inexact : il s'agit d'une fenêtre fixe depuis la première requête, `virtual_key.rs:79-91`).

---

### 🟡 F-19 — Remapping silencieux des modèles vers des cibles arbitraires

* **Sévérité** : Moyenne
* **Fichier** : `crates/pylos-application/src/use_cases/inference.rs:742-845`

Lorsqu'un provider ne supporte pas le modèle demandé, `map_model_for_provider` substitue un modèle **codé en dur** selon des sous-chaînes (`flash`/`mini`/`pro`/`gpt-4`). Exemples : toute requête non reconnue vers OpenAI devient `gpt-4o` ou `gpt-4o-mini` ; vers Anthropic, `claude-haiku-3-5`/`claude-3-5-sonnet`. La réponse renvoyée expose le modèle réellement utilisé, mais le client peut voir sa requête exécutée sur un modèle différent du modèle demandé, sans échec explicite — avec des impacts de coût et de qualité non maîtrisés. Certains identifiants cibles (`deepseek-v4-pro`, `gemini-3.5-pro`) ne correspondent à aucun modèle public connu.

**Recommandation** : rendre les tables de correspondance configurables (`pylos.json`), retourner 400 `model_not_supported` plutôt que de substituer implicitement, ou exiger un opt-in explicite du client.

---

### 🟡 F-20 — 211 `unwrap()` dans les crates, dont des chemins d'exécution

* **Sévérité** : Moyenne
* **Fichiers** : `crates/pylos-application/src/prefix_cache.rs` (15), `.../providers/gemini/provider.rs` (12), `.../guardrails.rs` (9), `.../rate_limit_store.rs` (8), `.../model_catalog.rs` (7), `.../virtual_key_store.rs` (7), `.../config_store.rs` (6), `crates/pylos-server/src/interfaces/http/inference.rs` (4)…

La majorité est dans les tests, mais plusieurs occurrences sont dans le code de production (sérialisation JSON, verrous, parsing). Avec `panic = "abort"` en release (`Cargo.toml:52`), un `unwrap` qui échoue **tue le processus** au lieu de dégrader l'erreur d'une requête.

**Recommandation** : instaurer `#![deny(clippy::unwrap_used, clippy::expect_used)]` (au moins hors tests) ou une règle CI dédiée.

---

### 🔵 F-21 — Constats secondaires (faible / informatifs)

| # | Constat | Fichier |
|---|---|---|
| a | Comparaison non constant-time du hash admin (incohérent avec `management_auth`) | `middleware/admin_guard.rs:24-25,41-42` |
| b | Valeurs de VK prédictibles si `value` absente (`sk-pylos-{id}`) ; `fastrand` (non CSPRNG) pour générer les clés OAuth | `state.rs:710-716,737-739`, `auth.rs:243` |
| c | Artefacts versionnés : `pylos-*.db`, `*-wal`, `*-shm`, `pylos.log`, `ci_test.log` (177 Ko de logs CI), `megalinter-reports/` (dont logs **-ERROR**) | racine du dépôt |
| d | `cargo fmt --check` désactivé dans tous les workflows | `ci.yml:26-27`, `run-tests.sh:36-44` |
| e | `cargo audit` silencieusement ignoré si l'outil est absent (le job reste vert) | `run-tests.sh:71-80` |
| f | `docker-compose` expose 3000/8080/11434 sur toutes les interfaces et utilise des images `:latest` non épinglées | `docker-compose.yml:27-28,69-70,81,84-85` |
| g | Comptage des tokens en streaming estimé par `len/4` → coûts/budgets imprécis | `interfaces/http/inference.rs:352-361` |
| h | Le disjoncteur ajoute les providers « ouverts » en fin de chaîne, qui sont donc quand même essayés ; pas d'état half-open explicite | `use_cases/inference.rs:107-143,258-278` |

---

## 4. Vérifications exécutées

Commandes lancées sur le commit `9a01264` (Windows, Rust 1.98.1) :

| Commande | Résultat |
|---|---|
| `cargo check --workspace --all-targets` | ✅ Succès (1 min 16 s) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Succès, exit 0 (aucun avertissement) |
| `cargo test --workspace` | ✅ **103 tests passés, 0 échec** : application 45 + store_integration 4 + core 23 + infrastructure 18 + server 4 + inference 7 + management 2 + doc-tests 0 |
| Scan de secrets (regex `sk-*`, `ghp_*`, `AIza*`, `AKIA*`, clés privées PEM) sur l'arbre versionné | ✅ 0 correspondance |

Ces résultats confirment le journal CI `ci_test.log` (2026-09-04 : mêmes totaux). **Aucun test ne couvre toutefois les défauts F-01 à F-09** : la suite d'intégration `management.rs:test_management_api_auth` ne teste que l'acceptation de la clé admin, pas le refus des Virtual Keys ni des rôles non-admin.

### Vérification du rapport d'audit précédent

`pylos_incomplete_features_report.md` (28 juin 2026) conclut « 11 points, 11 implémentés, aucun problème ouvert ». Les points fonctionnels qu'il revendique (coalescing de batching, boucle de correction structured output, RAG configurable, vrai prefix cache) sont bien présents dans le code — la revue n'a pas trouvé de contradiction sur ces 11 items. En revanche, ce rapport est une **auto-évaluation fonctionnelle** : il ne couvre ni l'autorisation, ni la gouvernance, ni la protection des données. Sa conclusion « Aucun problème ouvert » est démentie par la présente revue.

---

## 5. Points forts

1. **Architecture** : séparation nette core / application / infrastructure / server, traits `Provider`/`LlmPlugin`, injection de dépendances par `AppState` — le code est lisible et testable.
2. **SQL** : 100 % des requêtes observées sont paramétrées (`$1`, `?1`), y compris la construction dynamique de `WHERE` (`log_store.rs:668-711`) — aucune injection détectable.
3. **Absence de `unsafe`** : 0 occurrence dans les 108 fichiers Rust.
4. **Qualité d'outillage** : clippy `-D warnings` vert, CodeQL, MegaLinter, pre-commit, tests d'intégration par API (Axum `oneshot`).
5. **Redaction** : `list_providers` et `redact_config` masquent bien les valeurs de clés dans les réponses API (`config.rs:62-87,606-624`).
6. **Chiffrement des Virtual Keys** : AES-256-GCM avec nonce aléatoire et test de non-déterminisme (`encryption.rs:70-101`).
7. **Déploiement** : Helm avec `securityContext.runAsNonRoot`, secrets externalisés (`existingSecret`, External Secrets), `promote` protégé par `GH_PAT`.
8. **Honnêteté** : l'avertissement « Not Production Ready » du README est conforme à l'état observé.

---

## 6. Plan d'action recommandé

### Sprint 0 — avant toute exposition (bloquant)
1. **F-01 / F-02** : unifier l'autorisation du plan de contrôle — suppression de la branche Virtual Key, exigence du rôle admin relu depuis le store. Ajouter les tests négatifs correspondants.
2. **F-03 / F-05** : supprimer `x-bypass-guardrails` et valider/verrouiller `x-pylos-force-*`.
3. **F-04** : arrêter de journaliser les messages bruts et de persister `pii_mapping`.
4. **F-09** : révoquer le PAT GitHub et nettoyer le remote.
5. **F-06 / F-08** : corriger le câblage id/budget et le déclenchement du fallback.

### Sprint 1 — durcissement (sous 30 jours)
6. **F-10** (Argon2id), **F-11** (clé de chiffrement obligatoire, `OnceLock`), **F-12** (expiration VK), **F-13** (clés providers chiffrées), **F-14** (OAuth `state` + `ADMIN_EMAILS`), **F-07 / F-15** (liste blanche d'en-têtes, endpoints protégés).

### Sprint 2 — qualité et gouvernance
7. **F-16 / F-17** : rendre le blocage effectif et corriger le streaming ; **F-18** : unifier le rate limiting ; **F-19** : remapping configurable ; **F-20** : bannir `unwrap` hors tests.
8. Hygiène : retirer les bases SQLite/WAL/logs/megalinter du suivi git, activer `cargo fmt --check`, épingler les images Docker.

---

## 7. Conclusion

Pylos est un projet **techniquement solide et bien outillé**, mais dont le **plan de contrôle n'est pas sûr en l'état** : deux chemins d'escalade de privilèges (F-01, F-02) permettent à un locataire ou à un utilisateur simple d'administrer la passerelle, et plusieurs garde-fous annoncés (guardrails, budgets, restrictions de provider, expiration) sont contournables ou inopérants (F-03 à F-06, F-12). Le classement « non prêt pour la production » du README est donc exact, et la mise en production devrait être conditionnée à la clôture du Sprint 0 ci-dessus.

---

*Rapport généré automatiquement à partir de l'analyse statique et de l'exécution de la suite de tests sur le commit `9a01264`.*
