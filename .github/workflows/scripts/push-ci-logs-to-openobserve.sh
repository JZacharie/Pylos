#!/usr/bin/env bash
#
# Expédie les logs d'un run GitHub Actions vers OpenObserve (O2).
#
# Appelé par .github/workflows/ci-logs.yml, lui-même déclenché à la fin
# (« workflow_run: completed ») des workflows CI, PR Tests, MegaLinter et CodeQL.
#
# Variables d'environnement attendues :
#   GITHUB_TOKEN            jeton de lecture des logs (secrets.GITHUB_TOKEN)
#   OPENOBSERVE_ENDPOINT    URL d'ingestion, ex.
#                           https://o2-openobserve.p.zacharie.org/api/default/githubactions/_json
#   OPENOBSERVE_ACCESS_KEY  base64 de « email:mot-de-passe » (en-tête Authorization: Basic)
#   RUN_ID                  identifiant du run à expédier (github.event.workflow_run.id)
#   GITHUB_REPOSITORY       propriétaire/dépôt (fourni par le runner)
#   BATCH_SIZE              (optionnel) lignes par requête HTTP, défaut 1000
#
# Le contenu des logs est NON FIABLE : il n'est jamais évalué par le shell.
# Il transite uniquement par des fichiers et des arguments jq.

set -euo pipefail

: "${GITHUB_TOKEN:?GITHUB_TOKEN manquant}"
: "${OPENOBSERVE_ENDPOINT:?OPENOBSERVE_ENDPOINT manquant}"
: "${OPENOBSERVE_ACCESS_KEY:?OPENOBSERVE_ACCESS_KEY manquant}"
: "${RUN_ID:?RUN_ID manquant}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY manquant}"

BATCH_SIZE="${BATCH_SIZE:-1000}"

# gh privilégie GH_TOKEN ; on le laisse intact s'il est déjà défini.
export GH_TOKEN="${GH_TOKEN:-$GITHUB_TOKEN}"
export NO_COLOR=1

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

failures=0
records_total=0

# --- Métadonnées du run ------------------------------------------------------
if ! gh api "/repos/${GITHUB_REPOSITORY}/actions/runs/${RUN_ID}" > "${WORKDIR}/run.json"; then
  echo "::error::Impossible de lire le run ${RUN_ID}" >&2
  exit 1
fi

run_str() { jq -r "${1} // \"\"" "${WORKDIR}/run.json"; }
run_num() { jq -r "${1} // 0" "${WORKDIR}/run.json"; }

RUN_NAME="$(run_str '.name')"
RUN_NUMBER="$(run_num '.run_number')"
RUN_ATTEMPT="$(run_num '.run_attempt')"
RUN_EVENT="$(run_str '.event')"
RUN_REF="$(run_str '.head_branch')"
RUN_SHA="$(run_str '.head_sha')"
RUN_ACTOR="$(run_str '.actor.login')"
RUN_CONCLUSION="$(run_str '.conclusion')"

echo "Run #${RUN_NUMBER} « ${RUN_NAME} » (${RUN_CONCLUSION}) — tentative ${RUN_ATTEMPT}"

# --- Liste des jobs ----------------------------------------------------------
if ! gh api --paginate \
  "/repos/${GITHUB_REPOSITORY}/actions/runs/${RUN_ID}/jobs?per_page=100" \
  --jq '.jobs[] | @base64' > "${WORKDIR}/jobs.b64"; then
  echo "::error::Impossible de lister les jobs du run ${RUN_ID}" >&2
  exit 1
fi

# Envoie un fichier NDJSON par lots ; met à jour failures/records_total.
# OpenObserve répond 200 même lorsqu'il rejette des enregistrements : on analyse
# donc le corps de la réponse, pas seulement le code HTTP, pour ne jamais perdre
# des logs en silence.
push_records() {
  local file="$1"
  local chunk payload response sent=0 accepted rejected

  [ -s "$file" ] || return 0

  split -l "$BATCH_SIZE" -a 4 "$file" "${WORKDIR}/chunk_"
  for chunk in "${WORKDIR}"/chunk_*; do
    [ -e "$chunk" ] || continue
    payload="${WORKDIR}/payload.json"
    jq -c -s . "$chunk" > "$payload"

    if response="$(curl --silent --show-error --fail-with-body \
      --retry 3 --retry-delay 2 --max-time 120 \
      -X POST "$OPENOBSERVE_ENDPOINT" \
      -H "Authorization: Basic ${OPENOBSERVE_ACCESS_KEY}" \
      -H "Content-Type: application/json" \
      --data-binary "@${payload}")"; then
      accepted="$(jq -r '[.status[]?.successful // 0] | add // 0' <<<"$response" 2> /dev/null || echo 0)"
      rejected="$(jq -r '[.status[]?.failed // 0] | add // 0' <<<"$response" 2> /dev/null || echo 0)"
      if [ "${rejected}" -gt 0 ]; then
        echo "::warning::OpenObserve a rejeté ${rejected} ligne(s) : $(jq -r '[.status[]?.error // empty] | unique | join("; ")' <<<"$response" 2> /dev/null | cut -c1-300)" >&2
        failures=$((failures + 1))
      fi
      sent=$((sent + ${accepted:-0}))
    else
      echo "::warning::Échec de l'envoi d'un lot de $(wc -l < "$chunk") ligne(s) vers OpenObserve (erreur HTTP)" >&2
      failures=$((failures + 1))
    fi
    rm -f "$chunk"
  done

  records_total=$((records_total + sent))
  echo "  → ${sent} ligne(s) acceptée(s) par OpenObserve"
}

# --- Un enregistrement par ligne de log --------------------------------------
while IFS= read -r job_b64; do
  [ -n "$job_b64" ] || continue

  job_json="$(printf '%s' "$job_b64" | base64 --decode)"
  job_id="$(printf '%s' "$job_json" | jq -r '.id')"
  job_name="$(printf '%s' "$job_json" | jq -r '.name')"
  job_conclusion="$(printf '%s' "$job_json" | jq -r '.conclusion // ""')"
  job_completed="$(printf '%s' "$job_json" | jq -r '.completed_at // ""')"

  # Horodatage de repli (microsecondes) pour les lignes sans date exploitable.
  if [ -n "$job_completed" ]; then
    fallback_us="$(date -u -d "$job_completed" +%s%6N)"
  else
    fallback_us="$(date -u +%s%6N)"
  fi

  echo "Job « ${job_name} » (${job_conclusion:-inconnu})"

  # curl plutôt que « gh api » : gh refuse de restituer un log contenant des
  # séquences d'échappement ANSI (sorties colorées), ce qui faisait perdre des
  # jobs entiers. curl suit la redirection 302 vers le stockage des logs.
  if ! curl --silent --show-error --fail-with-body --location \
    --retry 3 --retry-delay 2 --max-time 120 \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -H "Accept: application/vnd.github+json" \
    "${GITHUB_API_URL:-https://api.github.com}/repos/${GITHUB_REPOSITORY}/actions/jobs/${job_id}/logs" \
    > "${WORKDIR}/job.log" 2> "${WORKDIR}/job.err"; then
    echo "::warning::Log indisponible pour le job « ${job_name} » (${job_id}) : $(tr -d '\n' < "${WORKDIR}/job.err" | cut -c1-200)" >&2
    continue
  fi

  jq -R -s -c \
    --arg workflow "$RUN_NAME" \
    --arg job "$job_name" \
    --argjson job_id "$job_id" \
    --argjson run_id "$RUN_ID" \
    --argjson run_number "$RUN_NUMBER" \
    --argjson run_attempt "$RUN_ATTEMPT" \
    --arg event "$RUN_EVENT" \
    --arg repo "$GITHUB_REPOSITORY" \
    --arg ref "$RUN_REF" \
    --arg sha "$RUN_SHA" \
    --arg actor "$RUN_ACTOR" \
    --arg run_conclusion "$RUN_CONCLUSION" \
    --arg job_conclusion "$job_conclusion" \
    --argjson fallback_us "$fallback_us" '
      split("\n")
      | map(select(test("\\S")))
      | map(
          . as $line
          | ($line[0:19] + "Z" | try fromdateiso8601 catch null) as $sec
          | ($line | gsub("\u001b\\[[0-9;]*[A-Za-z]"; "")) as $clean
          | {
              _timestamp: (if $sec == null then $fallback_us else ($sec * 1000000) end),
              workflow: $workflow,
              job: $job,
              job_id: $job_id,
              run_id: $run_id,
              run_number: $run_number,
              run_attempt: $run_attempt,
              event: $event,
              repo: $repo,
              ref: $ref,
              sha: $sha,
              actor: $actor,
              run_conclusion: $run_conclusion,
              job_conclusion: $job_conclusion,
              level: (
                if ($clean | test("##\\[error\\]")) then "error"
                elif ($clean | test("##\\[warning\\]")) then "warning"
                else "info" end
              ),
              message: $clean
            }
        )
      | .[]
    ' "${WORKDIR}/job.log" > "${WORKDIR}/job-records.ndjson"

  push_records "${WORKDIR}/job-records.ndjson"
done < "${WORKDIR}/jobs.b64"

if [ "$failures" -gt 0 ]; then
  echo "::error::${failures} lot(s) n'ont pas pu être envoyés à OpenObserve" >&2
  exit 1
fi

echo "OpenObserve : ${records_total} ligne(s) acceptée(s) pour le run ${RUN_ID}."
