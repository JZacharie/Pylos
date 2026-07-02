# syntax=docker/dockerfile:1
# ─────────────────────────────────────────────────────────────────────────────
# Stage 0 — Get Cross-Compilation Helpers
# ─────────────────────────────────────────────────────────────────────────────
FROM --platform=$BUILDPLATFORM tonistiigi/xx AS xx

# ─────────────────────────────────────────────────────────────────────────────
# Stage 1 — Builder
# Utilise l'image officielle Rust avec les dépendances système nécessaires,
# configurée pour la cross-compilation multi-architecture
# ─────────────────────────────────────────────────────────────────────────────
FROM --platform=$BUILDPLATFORM rust:1.95-bookworm AS builder
COPY --from=xx / /

# Disable CPU Jitter entropy in aws-lc-sys to avoid cross-compilation errors with clang/cc-rs optimization flags
ENV AWS_LC_SYS_NO_JITTER_ENTROPY=1
ENV PKG_CONFIG_ALLOW_CROSS=1

WORKDIR /build

# Dépendances système de l'hôte pour la compilation (Clang, LLD et pkg-config)
RUN apt-get update && apt-get install -y --no-install-recommends \
    clang \
    lld \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# ─── Couche cache des dépendances ────────────────────────────────────────────
# On copie uniquement les manifestes d'abord pour bénéficier du cache Docker.
# Tant que Cargo.toml/Cargo.lock ne changent pas, cette couche est réutilisée.
COPY Cargo.toml Cargo.lock ./

# Création de stubs vides pour chaque crate du workspace
RUN mkdir -p \
    crates/pylos-core/src \
    crates/pylos-application/src \
    crates/pylos-infrastructure/src \
    crates/pylos-server/src

COPY crates/pylos-core/Cargo.toml         crates/pylos-core/Cargo.toml
COPY crates/pylos-application/Cargo.toml  crates/pylos-application/Cargo.toml
COPY crates/pylos-infrastructure/Cargo.toml crates/pylos-infrastructure/Cargo.toml
COPY crates/pylos-server/Cargo.toml       crates/pylos-server/Cargo.toml

# Stubs minimaux pour pré-compiler les dépendances
RUN echo "fn main() {}" > crates/pylos-server/src/main.rs && \
    echo "pub fn init() {}" > crates/pylos-core/src/lib.rs && \
    echo "pub fn init() {}" > crates/pylos-application/src/lib.rs && \
    echo "pub fn init() {}" > crates/pylos-infrastructure/src/lib.rs

# Déclaration de la plateforme cible et installation des librairies cibles correspondantes
ARG TARGETPLATFORM
RUN apt-get update && xx-apt-get install -y --no-install-recommends \
    gcc \
    libc6-dev \
    && rm -rf /var/lib/apt/lists/*

RUN xx-clang --setup-target-triple

# Pré-compilation des dépendances uniquement (layer cachée) pour la cible
RUN PKG_CONFIG=xx-pkg-config xx-cargo build --release -p pylos-server 2>&1 | tail -5 || true

# ─── Build réel ──────────────────────────────────────────────────────────────
# On supprime les artefacts des stubs pour forcer la recompilation du code réel
RUN rm -rf \
    target/*/release/.fingerprint/pylos-* \
    target/*/release/deps/pylos_* \
    target/*/release/pylos-*

# Copie de tout le code source
COPY crates/ crates/
COPY rustfmt.toml ./

# Le crate pylos-server embarque l'UI via rust-embed (ui/dist).
# On crée un répertoire vide pour que la macro ne plante pas, même si l'UI
# n'est pas construite dans cette image (le serveur sert alors une page
# d'avertissement).
RUN mkdir -p ui/dist

# Build de release pour la cible, et copie dans un chemin neutre
# xx-cargo utilise --target ce qui crée un sous-répertoire dans target/.
# On cherche explicitement dans target/*/release/ puis target/release/
RUN PKG_CONFIG=xx-pkg-config xx-cargo build --release -p pylos-server && \
    src="$(ls target/*/release/pylos-server 2>/dev/null | head -1)" && \
    if [ -z "$src" ]; then src="target/release/pylos-server"; fi && \
    if [ -f "$src" ]; then \
        cp "$src" ./pylos-server; \
    else \
        echo "ERROR: pylos-server not found in target/"; \
        ls -d target/*/release/ 2>/dev/null || echo "(no target subdirs)"; \
        exit 1; \
    fi

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2 — Runtime
# Image minimale Debian slim : accès aux libs système (libssl, libc) sans Rust
# ─────────────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Dépendances runtime uniquement
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copie du binaire compilé
COPY --from=builder /build/pylos-server /app/pylos-server

# Utilisateur non-root pour la sécurité
RUN useradd --system --uid 1001 --no-create-home pylos && \
    mkdir -p /data && chown pylos:pylos /data
USER pylos

# Variables d'environnement par défaut
ENV PORT=3000
ENV RUST_LOG=info

EXPOSE 3000

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/app/pylos-server"]
