# Performance & Benchmarks — Pylos

## Budget d'overhead cible
- **Objectif** : `< 2 ms` d'overhead CPU sur le chemin chaud de la passerelle.

## Mesures Criterion (`crates/pylos-application/benches/hot_path.rs`)

Exécution :
```bash
cargo bench -p pylos-application --bench hot_path
```

| Benchmark | Latence médiane mesurée | Part du budget (`2 ms`) |
|---|---|---|
| `routing/map_model_for_provider` | `27.48 ns` | `0.001%` |
| `cache/serialize_cache_key` | `164.83 ns` | `0.008%` |
| `serialization/chat_request_roundtrip` | `1.29 µs` | `0.065%` |
| `guardrails/normalize_and_moderate` | `4.41 µs` | `0.221%` |
| **Total chemin chaud** | **~5.87 µs** | **0.29%** |

> L'audit par benchmark a permis d'identifier et de corriger la recompilation des ~100 regex de modération à chaque appel dans `compute_moderation_scores` (`18.21 ms` → `4.41 µs` via `LazyLock`).
