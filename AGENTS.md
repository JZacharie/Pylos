# Pylos — Guide Agent & Développeur (`AGENTS.md`)

Ce fichier définit les conventions d'ingénierie, de test et d'hygiène Git pour tout agent LLM ou contributeur intervenant sur **Pylos**.

## 1. Commandes de Vérification (Obligatoires avant Commit)

### Rust (Backend)
Sur Windows, définir `AWS_LC_SYS_NO_ASM=1` et `PYLOS_BUILD_DATE` si besoin :
```powershell
$env:AWS_LC_SYS_NO_ASM="1"
$env:PYLOS_BUILD_DATE="2026-11-14"
cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo test --workspace
```
- **Zéro `unwrap()` / `expect()`** en code de production (`#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]`).

### Frontend (UI React)
```powershell
npm --prefix ui run build
npm --prefix ui run test -- --run
npm --prefix ui run lint
```

## 2. Hygiène Git & Branches
- **`main`** est la branche unique de référence.
- Créer des branches de travail courtes dédiées par issue/PR (`feat/<id>-...`, `fix/<id>-...`).
- `deleteBranchOnMerge: true` est activé sur `JZacharie/Pylos` : toute branche mergée est supprimée automatiquement.
- Ne jamais contourner le hook `Secret-Guard`.

## 3. Observabilité & CI/CD
- Les workflows `.github/workflows/` sont organisés par phases (`00` Sécurité -> `01` Lint -> `02` Tests PR/Main -> `03` CodeQL -> `04` Release/Pages -> `05` Télémétrie OpenObserve).
- Les logs et métriques structurées (`phase`, `duration_ms`, `job_metric`, `run_metric`) sont expédiés vers OpenObserve (`githubactions`) avec le dashboard v8 `docs/openobserve/pylos-ci-observability.v8.json`.

## 4. Règles de Routage & FinOps
- Prioriser systématiquement les providers gratuits / à quota d'abonnement (`Free` / `QuotaSubscription`) avant le fallback payant (`Paid`).
- Exclure immédiatement les providers `Unavailable` / `Saturated` (`503 ServiceUnavailable` si aucun provider autorisé n'est disponible).
- Maintenir `CONTEXT.md` et `docs/adr/` à jour lors de toute évolution structurante.
