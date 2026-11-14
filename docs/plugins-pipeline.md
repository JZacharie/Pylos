# Pipeline de Plugins & Streaming — Pylos

## Ordre explicite par priorité (`LlmPlugin::priority`)

`InferenceOrchestrator::new` trie de manière déterministe tous les plugins par priorité croissante (`priority() -> i32`) :

- **`pre_hook`** : exécuté par priorité croissante (du plus petit au plus grand).
- **`post_stream_chunk`** & **`post_hook`** : exécutés en ordre inverse (LIFO par priorité décroissante).

| Priorité | Plugin | Rôle |
|---|---|---|
| `-80` | `rate_limit` | Limitation RPM/TPM |
| `-70` | `budget` | Contrôle et décompte budgétaire |
| `-50` | `guardrails` | Modération, blocage, masquage PII (`pre_hook`) et démasquage PII (`post_hook` / `post_stream_chunk`) |
| `0` | Caches / Routage (défaut) | `prefix_cache`, `semantic_cache`, `smart_router`, etc. |
| `50` | `structured_output` | Validation / correction JSON Schema |
| `100` | `otel` | Télémétrie OpenTelemetry |

## Streaming et Post-Hooks

1. **Démasquage PII en temps réel (`post_stream_chunk`)** : lorsqu'un mapping PII (`x-pii-mapping`) est actif, `wrap_stream_with_post_hooks` retient uniquement un éventuel préfixe de placeholder incomplet `[...` en fin de chunk afin de réassembler les placeholders coupés entre deux chunks SSE avant d'appliquer `post_stream_chunk` et d'envoyer au client.
2. **Budget & métriques exacts** : l'objet `Usage` du dernier chunk (ou une estimation de repli si le provider n'émet pas `usage` en streaming) est transmis à la réponse reconstruite avant l'exécution de `post_hook`.
