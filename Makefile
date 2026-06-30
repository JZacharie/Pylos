.DEFAULT_GOAL := help

.PHONY: help setup build build-dev run run-config test test-verbose fmt fmt-check lint audit deny docker-build docker-up docker-down ui-dev ui-build clean all

define check_tool
	@command -v $(1) >/dev/null 2>&1 || { echo "Error: $(1) is not installed. Please install it first."; exit 1; }
endef

help: ## Display this help message
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n\nTargets:\n"} \
		/^[a-zA-Z_-]+:.*##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

# ── Setup ─────────────────────────────────────────────────────────────────────
setup: ## Installe les outils de développement nécessaires
	$(call check_tool,cargo)
	rustup component add clippy rustfmt
	@echo "Installing cargo-audit (security audits)..."
	cargo install cargo-audit --quiet || true
	@echo "Installing cargo-deny (dependency policy)..."
	cargo install cargo-deny --quiet || true
	@echo "Setup complete."

# ── Build ─────────────────────────────────────────────────────────────────────
build: ui-build ## Compile en mode release avec l'UI intégrée
	$(call check_tool,cargo)
	cargo build --release -p pylos-server

build-dev: ui-build ## Compile en mode debug (rapide) avec l'UI intégrée
	$(call check_tool,cargo)
	cargo build -p pylos-server

build-backend: ## Compile le backend seul (sans rebuild UI)
	$(call check_tool,cargo)
	cargo build --release -p pylos-server

build-backend-dev: ## Compile le backend seul en debug (sans rebuild UI)
	$(call check_tool,cargo)
	cargo build -p pylos-server

# ── Run ───────────────────────────────────────────────────────────────────────
run: ui-build ## Lance le serveur en mode dev (rechargement manuel) avec UI intégrée
	$(call check_tool,cargo)
	RUST_LOG=info cargo run -p pylos-server

run-config: ui-build ## Lance le serveur avec un fichier de config spécifique
	$(call check_tool,cargo)
	RUST_LOG=info cargo run -p pylos-server -- --config pylos.json

# ── Test ──────────────────────────────────────────────────────────────────────
test: ## Lance tous les tests unitaires et d'intégration
	$(call check_tool,cargo)
	cargo test

test-verbose: ## Lance les tests avec output verbose
	$(call check_tool,cargo)
	cargo test -- --nocapture

# ── Lint & Format ─────────────────────────────────────────────────────────────
fmt: ## Applique le formattage automatique
	$(call check_tool,cargo)
	cargo fmt --all

fmt-check: ## Vérifie le formattage sans modification
	$(call check_tool,cargo)
	cargo fmt --all -- --check

lint: ## Lance clippy (lint)
	$(call check_tool,cargo)
	cargo clippy --all-targets --all-features -- -D warnings

# ── Sécurité ──────────────────────────────────────────────────────────────────
audit: ## Audit de sécurité des dépendances (requiert cargo-audit)
	$(call check_tool,cargo)
	cargo audit

deny: ## Vérification des politiques de dépendances (requiert cargo-deny)
	$(call check_tool,cargo)
	cargo deny check

# ── Docker / Podman ───────────────────────────────────────────────────────────
CONTAINER_ENGINE := $(shell command -v podman 2>/dev/null || command -v docker 2>/dev/null)
COMPOSE_CMD := $(shell command -v podman-compose 2>/dev/null || echo "$(CONTAINER_ENGINE) compose")

docker-build: ## Build l'image container (auto-détecte podman/docker)
	@test -n "$(CONTAINER_ENGINE)" || { echo "Error: neither 'podman' nor 'docker' is installed."; exit 1; }
	$(CONTAINER_ENGINE) build -t pylos:latest .

docker-up: ## Lance la stack complète (gateway + UI + Prometheus + Grafana)
	@test -n "$(CONTAINER_ENGINE)" || { echo "Error: neither 'podman' nor 'docker' is installed."; exit 1; }
	$(COMPOSE_CMD) up --build -d

docker-down: ## Arrête la stack
	@test -n "$(CONTAINER_ENGINE)" || { echo "Error: neither 'podman' nor 'docker' is installed."; exit 1; }
	$(COMPOSE_CMD) down

docker-build-ssl: ## Build avec les certs SSL de l'hôte (pour proxy corporate)
	@test -n "$(CONTAINER_ENGINE)" || { echo "Error: neither 'podman' nor 'docker' is installed."; exit 1; }
	@echo "Copying host CA certificates into build context..."
	@cp /etc/ssl/certs/ca-certificates.crt docker/ca-certificates.crt 2>/dev/null || true
	$(CONTAINER_ENGINE) build -t pylos:latest .
	@rm -f docker/ca-certificates.crt

# ── UI ────────────────────────────────────────────────────────────────────────
ui-dev: ## Lance le serveur de dev UI
	$(call check_tool,npm)
	cd ui && npm run dev

ui-build: ## Build la UI pour la production
	$(call check_tool,npm)
	cd ui && npm run build

# ── Clean ─────────────────────────────────────────────────────────────────────
clean: ## Nettoie les artefacts de compilation
	$(call check_tool,cargo)
	cargo clean

# ── Pipeline CI/CD ────────────────────────────────────────────────────────────
all: fmt-check lint test ## Pipeline complet : format + lint + test
	@echo "All checks passed."
