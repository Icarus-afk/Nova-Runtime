.PHONY: help setup dev build test docker clean fmt lint sdk example check

DEFAULT_GOAL := help

# Single-command DX for Nova Runtime
# Usage: make setup && make dev
# Or:    make docker

help: ## Show this help
	@echo "Nova Runtime — unified backend (SQL, cache, queue, search, blob, auth)"
	@echo ""
	@echo "Usage:"
	@echo "  make setup    One-time setup (Rust + Node deps, builds novad+novactl, creates novad.toml) [2-3 min]"
	@echo "  make dev      Run backend + dashboard with live reload (port 8642 + 5173)"
	@echo "  make build    Release build (novad + novactl)"
	@echo "  make test     Run all tests (~1,500)"
	@echo "  make docker   Build and run via Docker Compose"
	@echo "  make sdk      Build TypeScript SDK"
	@echo "  make example  Run SDK quickstart example"
	@echo "  make fmt      Format Rust + dashboard"
	@echo "  make lint     Clippy + typecheck"
	@echo "  make clean    Remove build artifacts"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

setup: ## One-time setup: deps + debug builds + config
	@./scripts/setup.sh

dev: ## Run backend (cargo run) + dashboard (vite) concurrently
	@./scripts/dev.sh

build: ## Release build (all binaries)
	cargo build --release
	@echo "✓ binaries at target/release/novad and target/release/novactl"

test: ## Run workspace tests + SDK tests
	cargo test --workspace --exclude nova-sim
	cd sdk && npm test
	cd dashboard && npm run build

docker: ## Build and run via Docker (requires Docker)
	docker compose up --build

docker-build: ## Only build Docker image
	docker build -t nova-runtime .

sdk: ## Build SDK
	cd sdk && npm install && npm run build

example: sdk ## Run SDK quickstart against local novad (requires running novad)
	@node --loader ts-node/esm examples/quickstart.ts 2>/dev/null || npx tsx examples/quickstart.ts || echo "Install tsx: npm i -g tsx"

fmt: ## Format code
	cargo fmt --all
	cd dashboard && npx prettier --write "src/**/*.{ts,tsx}" 2>/dev/null || true
	cd sdk && npx prettier --write "src/**/*.ts" 2>/dev/null || true

lint: ## Lint (clippy + tsc)
	cargo clippy --workspace -- -D warnings
	cd dashboard && npm run build
	cd sdk && npm run build

check: ## Fast check (no build)
	cargo check --workspace

clean: ## Remove builds and data
	cargo clean
	rm -rf target dashboard/dist sdk/dist data
	@echo "✓ cleaned"

init-config: ## Generate commented novad.toml from template
	@cp -n novad.toml novad.toml.bak 2>/dev/null || true
	@./scripts/setup.sh --config-only
	@echo "✓ novad.toml created (see docs/02-configuration.md)"
