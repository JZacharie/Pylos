# Reprise du développement — guide pour un nouvel IDE

> **Public** : développeur qui reprend Pylos sur une nouvelle machine / un nouvel IDE.
> **Dernière mise à jour** : 11/10/2026 (branche `main`, PR #150 → #153 mergées).
> Ce document ne contient **aucun secret**. Voir §9 pour l'emplacement des secrets.

---

## 1. Prérequis

| Outil | Version | Nécessité |
| :--- | :--- | :--- |
| **Rust** (stable, via rustup) | 1.98+ | Obligatoire |
| **Node.js** | 22+ | Pour la console (`ui/`) |
| **Docker + buildx** | récent | Images, `docker compose`, MegaLinter local (optionnel) |
| **Git** | 2.40+ | Obligatoire |
| `gh` (GitHub CLI) | 2.50+ | PR, issues, CI (recommandé) |
| `kubectl` | — | Déploiement / Vault (optionnel) |

---

## 2. Clonage et authentification Git

```powershell
git clone https://github.com/JZacharie/Pylos.git
cd Pylos
```

> ⚠️ **Aucun token n'est stocké dans les remotes** (nettoyage effectué le 11/10/2026). Les URL
> sont propres : `https://github.com/JZacharie/Pylos.git`.

Authentification recommandée :

```bash
gh auth login              # puis : gh auth setup-git
# ou, si vous utilisez Git Credential Manager :
git config --global credential.helper manager   # déjà configuré au niveau système sur ce poste
```

**Un PAT ne doit jamais être collé dans l'URL du remote ni commité.** Voir §9.

---

## 3. Variables d'environnement minimales

```bash
cp .env.example .env
```

| Variable | Rôle | Sans elle |
| :--- | :--- | :--- |
| `PYLOS_ADMIN_KEY` | Clé d'administration statique (ou setup via la console) | Setup requis au premier lancement |
| `PYLOS_ENCRYPTION_KEY` | 32 octets base64 (`openssl rand -base64 32`) | Clé **éphémère** : valeurs chiffrées perdues au redémarrage ; **refus de démarrer** si `PYLOS_ENV=production` |
| `ADMIN_EMAILS` | Emails administrateurs (OAuth Google) | Aucun admin via OAuth |
| `PYLOS_METRICS_TOKEN` | Jeton Bearer exigé sur `/metrics` | `/metrics` en accès libre |
| `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, … | Aucune requise en test (MockProvider) | Pas d'inférence réelle |
| `DATABASE_URL` | PostgreSQL (multi-répliques) | SQLite locale |
| `RUST_LOG` | Verbosité (`info`, `debug`) | `info` |

Détails complets : `docs/pylos-config.md`, `docs/pylos-json-reference.md` et la page
[Sécurité & Exploitation](https://jzacharie.github.io/Pylos/securite/).

---

## 4. Pièges connus de l'environnement (spécifiques Windows)

| Symptôme | Cause | Contournement |
| :--- | :--- | :--- |
| `aws-lc-sys … NASM command not found` / `stdalign_check.c` compile error | NASM absent du `PATH` | `$env:AWS_LC_SYS_PREBUILT_NASM='1'` avant tout `cargo` (`AWS_LC_SYS_NO_JITTER_ENTROPY=1` est déjà posé dans le Dockerfile) |
| `error: failed to run custom build command for rust-embed` | Le binaire embarque `ui/dist` | `mkdir -p ui/dist` (stub) avant `cargo build/check/clippy` |
| `warning: LF will be replaced by CRLF` | Normalisation Git sous Windows | Bénin ; ne pas renormaliser sans raison |
| `cargo fmt --all -- --check` échoue | Dérive de formatage préexistante (assumée, désactivée en CI) | Ne pas l'activer sans une PR dédiée `cargo fmt --all` |
| `npm run lint --prefix ui` échoue (48 erreurs) | Dette ESLint existante (suivi **#134**) | La CI le laisse en `continue-on-error` |
| `git push` demande un mot de passe | Aucun identifiant en cache | §2 |

---

## 5. Commandes de développement

```powershell
# Rust
cargo check --workspace --all-targets          # compilation rapide
cargo clippy --workspace --all-targets -- -D warnings   # ce que la CI exige
cargo test --workspace                          # 125 tests
cargo test -p pylos-server --test inference     # tests d'intégration ciblés

# UI (console React)
npm ci --prefix ui
npm run build --prefix ui
npm run dev --prefix ui                         # Vite en dev

# Suite complète locale (SQLite, sans Docker)
.local-ci.ps1          # ou ./local-ci.sh / ./test-local.sh

# Stack Docker complète (gateway + UI + observabilité)
docker compose up -d            # UI : http://localhost:8080

# Déploiement Kubernetes (chart Helm)
helm template pylos ./helm/pylos
```

---

## 6. Architecture en 30 secondes

```text
pylos-core          domaine : entités, traits (Provider, LlmPlugin), config, erreurs
     ▲
pylos-infrastructure adaptateurs providers : openai, anthropic, gemini, cohere, bedrock, azure
     ▲
pylos-application   cas d'usage (orchestrateur), stores, plugins (budget, quotas, guardrails,
                    cache, RAG, batching, compression, mémoire)
     ▲
pylos-server        Axum : routes, middlewares (auth admin / Virtual Key), MCP, UI embarquée
```

Chemins de lecture recommandés pour comprendre une requête :
1. `crates/pylos-server/src/routes.rs` — cartographie des routes et middlewares ;
2. `crates/pylos-server/src/interfaces/http/inference.rs` — handler `/v1/chat/completions` ;
3. `crates/pylos-application/src/use_cases/inference.rs` — pipeline plugins → routage → providers → fallback ;
4. `crates/pylos-application/src/middleware/virtual_key.rs` (data-plane) et `middleware/management_auth.rs` (plan de contrôle).

Documentation de fond : `docs/review/` (revue de sécurité, revue d'architecture, plans d'action)
et le site <https://jzacharie.github.io/Pylos/>.

---

## 7. Branches, PR et protection de `main`

`main` est **protégée** :
- force-push et suppression interdits ;
- **3 checks requis** : `Run Tests`, `MegaLinter`, `Analyze (rust)` ;
- branche à jour obligatoire (`strict`) ;
- `enforce_admins=false` → un administrateur peut pousser directement (à éviter).

Convention de travail :

```bash
git checkout -b fix/mon-sujet origin/main
# … commits …
gh pr create --base main --head fix/mon-sujet --fill
# le corps de la PR doit contenir : Closes #NNN
gh pr merge --merge            # après checks verts (+ "Update branch" si nécessaire)
```

- **Une PR = une issue** (`Closes #NNN` dans le corps).
- Messages de commit en français, style `fix(scope): …`, `docs(scope): …`, `ci: …`.
- Échappatoire CI : un commit contenant `--skip-ci` court-circuite le pipeline PR.

---

## 8. CI/CD — ce qui tourne, quand

| Workflow | Déclencheur | Contenu | Durée typique |
| :--- | :--- | :--- | :--- |
| **PR Tests** | PR | `Detect changes` (filtrage par périmètre) → `Rust Lint`, `Rust Tests`, `Security Audit`, `UI Build` **en parallèle** → agrégateur **`Run Tests`** | ~3 min (Rust), ~40 s (UI) |
| **MegaLinter** | PR + push main | Lint multi-langages (sur le diff en PR, tout le dépôt en push) | ~2-6 min |
| **CodeQL** | PR + push main + hebdo | Analyse de sécurité Rust (`Analyze (rust)`) | ~9 min à froid, moins avec cache |
| **CI** | push main | `Check` (clippy + tests) puis images Docker multi-arch (`amd64`/`arm64`), manifest, déploiement dev | ~10-15 min |
| **Documentation (GitHub Pages)** | push touchant `site/**` | Build Jekyll + publication | ~1 min |

Optimisations en place : cache cargo **partagé** (`Swatinem/rust-cache`, clé `pylos-rust`) entre PR/CI/CodeQL,
cache npm pour l'UI, cache registry Docker (`mode=max`), `concurrency` (annulation des runs obsolètes),
`paths-ignore` (les commits de documentation ne déclenchent ni tests ni images Docker).

**Ne pas renommer le job agrégateur `Run Tests`** : c'est le nom du check requis par la protection
de branche.

---

## 9. Secrets et infrastructure

| Élément | Emplacement |
| :--- | :--- |
| Secrets applicatifs (clés providers, `pat_token`, `encryption_key`, OAuth…) | **HashiCorp Vault** : `secret/ai/pylos` (mount KV `secret`) |
| Consommation par Kubernetes | `ExternalSecret` `pylos-secrets` (namespace `pylos-dev`) via `ClusterSecretStore vault-backend` |
| Écriture dans Vault | `kubectl exec -n vault vault-0 -- sh -c "VAULT_TOKEN=… vault kv patch secret/ai/pylos <clé>='<valeur>'"` (le rôle Kubernetes `schema-vault-role` est en **lecture seule**) |
| Fichier local | `.env` (gitignoré) ; `pylos.json` (gitignoré) |

> ❌ **Ne jamais** commiter un token, une clé d'API ou un `pylos.json` réel.
> Un PAT présent dans un dépôt public est **révoqué automatiquement par GitHub** : c'est la
> conséquence observée le 11/10/2026 (jeton passé de fonctionnel à `401`).

---

## 10. Reprise sur un IDE neuf — checklist

1. `git clone` + `gh auth login` (§2).
2. `cp .env.example .env` et renseigner `PYLOS_ENCRYPTION_KEY` + `PYLOS_ADMIN_KEY` (§3).
3. `mkdir ui/dist` (stub rust-embed), `npm ci --prefix ui` (§4-5).
4. `cargo check --workspace --all-targets` → doit passer (sinon : `AWS_LC_SYS_PREBUILT_NASM=1`).
5. `cargo clippy --workspace --all-targets -- -D warnings` puis `cargo test --workspace` (125 tests).
6. `npm run build --prefix ui`.
7. Lire `docs/review/pylos-architecture-features-review-2026-10-11.md` (état de l'architecture) et
   `docs/review/pylos-backlog-action-plan-2026-10-11.md` (plan et jalons).
8. Ouvrir les jalons GitHub #1 → #6 et choisir une issue **`priority:p0`/`p1`** non terminée.
9. Créer la branche, développer, `gh pr create` avec `Closes #NNN`.
10. Vérifier les 3 checks requis avant de merger (le CI fait le reste).

### Où en est le plan (au 11/10/2026)

| Sprint | État |
| :--- | :--- |
| **Sprint 0** (sécurité bloquante #112-#118) | ✅ mergé (PR #150) |
| **Sprint 1** (gouvernance & fiabilité #119-#126) | ✅ mergé (PR #152), 8 issues fermées |
| **Sprint 2** (architecture & qualité #127-#135) | ⏳ à démarrer |
| **Sprint 3** (fonctionnalités & observabilité #136-#141) | ⏳ |
| **MCP Tool Execution Gateway** (jalon #5, epics #142-#148) | ⏳ prochaine étape : story **S-MCP-00** (transport `McpServer` + migrations) |
| **Mnemosyne pipeline observability** (#47, jalon #6) | ⏳ plan en 4 phases |

---

## 11. Liens utiles

- Site de documentation : <https://jzacharie.github.io/Pylos/>
- Issues et jalons : <https://github.com/JZacharie/Pylos/issues> · <https://github.com/JZacharie/Pylos/milestones>
- Dépôt : <https://github.com/JZacharie/Pylos>

---

*Guide généré le 11/10/2026. Si une commande ou une variable a changé, corriger ce fichier dans la même PR que le changement.*
