# ADR-0001 : Profil Release (`opt-level = "z"` vs `3` et `lto`)

## Statut
Accepté (2026-11-14)

## Contexte
`Cargo.toml` définit `[profile.release]` avec `opt-level = "z"`, `lto = "thin"`, `codegen-units = 4`, `strip = true`, `panic = "abort"`, ainsi qu'un profil `[profile.release-opt]` avec `lto = "fat"` et `codegen-units = 1`.
Le README annonce un overhead de passerelle `< 2 ms`, mais `opt-level = "z"` privilégie la taille du binaire (utile pour les images Docker et l'installeur MSI) plutôt que la vitesse brute.

## Mesures sur le chemin chaud (`cargo bench -p pylos-application --bench hot_path -- --quick`)
- `routing/map_model_for_provider` : **~27.5 ns**
- `cache/serialize_cache_key` : **~164.8 ns**
- `serialization/chat_request_roundtrip` : **~1.29 µs**
- `guardrails/normalize_and_moderate` (après précompilation `LazyLock` des regex) : **~4.41 µs** (contre `18.21 ms` avant correction)

Le chemin chaud complet représente **< 6 µs**, soit plus de **300×** en dessous du budget cible de `2 ms` (le coût réel de la passerelle étant dominé par le réseau et la persistance asynchrone, non par le CPU).

## Décision
1. **Conserver `opt-level = "z"` et `lto = "thin"`** sur `[profile.release]` pour les builds par défaut (images Docker multi-arch et binaire Windows compact, temps de compilation raisonnable en CI).
2. **Conserver `[profile.release-opt]`** (`lto = "fat"`, `codegen-units = 1`) pour les déploiements haute densité CPU.
3. **Maintenir `benches/hot_path.rs` en CI** (`cargo bench ... -- --test` / `--quick`) afin de détecter toute régression algorithmique (comme une recompilation de regex dans le chemin chaud).
