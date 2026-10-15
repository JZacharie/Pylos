# CI vers OpenObserve (O2)

Les logs de chaque run GitHub Actions (CI, PR Tests, MegaLinter, CodeQL) sont
expédiés vers OpenObserve, onglet **`githubactions`** de l'organisation
`default`.

| Élément | Valeur |
| --- | --- |
| Déclencheur | `.github/workflows/ci-logs.yml` (`workflow_run: completed`) |
| Expédition | `.github/workflows/scripts/push-ci-logs-to-openobserve.sh` |
| Endpoint | `https://o2-openobserve.p.zacharie.org/api/default/githubactions/_json` |
| Console | https://o2-openobserve.p.zacharie.org/web/logs?stream_type=logs&stream=githubactions&org_identifier=default |

## Fonctionnement

1. À la fin de chacun des workflows surveillés, GitHub déclenche
   `ci-logs.yml` (`workflow_run`).
2. Le script énumère les jobs du run via l'API GitHub, télécharge les logs de
   chaque job terminé et les convertit en un enregistrement par ligne.
3. Les enregistrements sont envoyés par lots de 1000 lignes (`BATCH_SIZE`)
   à l'endpoint d'ingestion O2, en HTTP Basic.

Chaque enregistrement contient :

| Champ | Exemple |
| --- | --- |
| `_timestamp` | date extraite de la ligne de log (µs), date de fin du job en repli |
| `workflow` | `CI`, `PR Tests`… |
| `job` / `job_id` | `Check` / `115682440392` |
| `run_id` / `run_number` / `run_attempt` | `38511978007` / `321` / `1` |
| `event` / `ref` / `sha` / `actor` | `push` / `main` / `e3c4515…` / `JZacharie` |
| `run_conclusion` / `job_conclusion` | `success` / `failure` |
| `level` | `error` / `warning` / `info` (déduit des marqueurs `##[error]` / `##[warning]`) |
| `message` | la ligne de log brute |

## Secrets de dépôt

À définir dans **Settings > Secrets and variables > Actions** :

| Secret | Contenu |
| --- | --- |
| `OPENOBSERVE_ENDPOINT` | `https://o2-openobserve.p.zacharie.org/api/default/githubactions/_json` |
| `OPENOBSERVE_ACCESS_KEY` | base64 de `<email-du-compte>:<token>`, à mettre dans l'en-tête `Authorization: Basic` |

Le secret `GITHUB_TOKEN` fourni par le runner suffit à lire les logs
(permission `actions: read` déclarée dans le workflow).

## Compte de service O2

L'ingestion utilise le compte de service dédié `pylos-ci.default@sa.internal`
(organisation `default`), créé pour la CI et distinct des comptes utilisés par
les applications (`llm.default@sa.internal`).

Rotation d'un token :

```bash
# Créer un nouveau compte de service et récupérer son token
curl -sS -u '<email-admin>:<mot-de-passe>' -H 'Content-Type: application/json' \
  -X POST https://o2-openobserve.p.zacharie.org/api/default/service_accounts \
  -d '{"email":"pylos-ci.default@sa.internal"}'

# Ré-encoder « compte:token » et mettre à jour le secret du dépôt
printf '%s' 'pylos-ci.default@sa.internal:<nouveau-token>' | base64 -w0
gh secret set OPENOBSERVE_ACCESS_KEY --repo JZacharie/Pylos --body '<base64>'
```

## Interroger les logs

Requête SQL depuis l'interface O2 (`stream_type=logs`, `stream=githubactions`) :

```sql
select workflow, job, job_conclusion, count(*) as lignes
from githubactions
group by workflow, job, job_conclusion
order by lignes desc
```

## Dépannage

- **`Too old data, only last 24 hours data can be ingested`** : OpenObserve
  refuse par défaut les enregistrements de plus de 24 h. En fonctionnement normal
  le `workflow_run` se déclenche quelques secondes après la fin du run, donc les
  logs sont acceptés ; ce message n'apparaît que si l'on rejoue un run ancien.
- **HTTP 200 avec `successful: 0`** : OpenObserve répond 200 même en cas de
  rejet. Le script analyse le corps de la réponse et échoue explicitement si
  `failed > 0`, pour ne jamais perdre de logs en silence.
- **Aucun déclenchement** : `workflow_run` ne fonctionne que si
  `.github/workflows/ci-logs.yml` est présent sur la branche par défaut.
