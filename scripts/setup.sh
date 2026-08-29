#!/usr/bin/env bash
set -euo pipefail

# Nova Runtime — one-command setup
# Usage: ./scripts/setup.sh [--release] [--config-only]
#   --release     : build --release (slower, optimized, ~5 min) instead of debug (fast, ~1 min)
#   --config-only : only (re)create novad.toml, skip builds
#   --help        : show help

RELEASE=0
CONFIG_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --release) RELEASE=1 ;;
    --config-only) CONFIG_ONLY=1 ;;
    --help|-h) echo "Usage: $0 [--release] [--config-only]"; exit 0 ;;
    *) echo "Unknown arg: $arg (try --help)"; exit 1 ;;
  esac
done

echo "=== Nova Runtime Setup ==="
echo "  Mode: $([ $RELEASE -eq 1 ] && echo release || echo debug) $([ $CONFIG_ONLY -eq 1 ] && echo '(config only)' || echo '')"
echo ""

# Prerequisites
need() { command -v "$1" >/dev/null 2>&1 || { echo "✗ $1 not found — $2"; exit 1; }; echo "✓ $1 $(command -v $1)"; }
echo "--- Checking prerequisites ---"
need cargo "Install from https://rustup.rs"
need node "Install from https://nodejs.org (18+)"
need npm "Install Node.js"
echo ""

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# Config
if [ ! -f "$ROOT_DIR/novad.toml" ] || [ $CONFIG_ONLY -eq 1 ]; then
    echo "--- Creating novad.toml (dev defaults, 8 lines) ---"
    cat > "$ROOT_DIR/novad.toml" << 'TOML'
# Nova Runtime — dev config (see docs/02-configuration.md for full reference)
# All values have sensible defaults — you can run with *no* config file.
# This file overrides defaults; delete any section to use defaults.
[general]
data_dir = "./data"

[networking]
listen_address = "127.0.0.1"
listen_port = 8642

# Uncomment for TLS:
# tls_enabled = true
# tls_cert_path = "./certs/cert.pem"
# tls_key_path = "./certs/key.pem"
TOML
    echo "✓ $ROOT_DIR/novad.toml"
    echo "  tip: full production template: cargo run --bin novactl -- config default > novad.full.toml"
else
    echo "✓ novad.toml exists (skip — use --config-only to overwrite)"
fi
echo ""

if [ $CONFIG_ONLY -eq 1 ]; then
    echo "=== Setup complete (config only) ==="
    exit 0
fi

# Backend — one command, no bin names to remember
echo "--- Building backend (cargo build) ---"
if [ $RELEASE -eq 1 ]; then
    cargo build --release 2>&1 | tail -5
    echo "✓ backend release build: target/release/novad and target/release/novactl"
else
    cargo build 2>&1 | tail -5
    echo "✓ backend debug build: target/debug/novad and target/debug/novactl (faster, use --release for prod)"
fi
echo ""

# Dashboard deps (only if dashboard present)
if [ -f "$ROOT_DIR/dashboard/package.json" ]; then
    echo "--- Installing dashboard dependencies (npm install) ---"
    cd "$ROOT_DIR/dashboard"
    npm install 2>&1 | tail -3
    echo "✓ dashboard deps"
    echo ""
fi

# SDK deps
if [ -f "$ROOT_DIR/sdk/package.json" ]; then
    echo "--- Installing SDK dependencies ---"
    cd "$ROOT_DIR/sdk"
    npm install 2>&1 | tail -3
    echo "✓ sdk deps"
    echo ""
fi

echo "=== Setup complete ==="
echo ""
echo "Binaries built:"
if [ $RELEASE -eq 1 ]; then
    echo "  target/release/novad    — daemon (./target/release/novad --help)"
    echo "  target/release/novactl  — CLI    (./target/release/novactl --help)"
else
    echo "  target/debug/novad    — daemon (cargo run --bin novad -- --help)"
    echo "  target/debug/novactl  — CLI    (cargo run --bin novactl -- --help)"
fi
echo "  Install to PATH:  make install  (copies to ~/.cargo/bin, ensure ~/.cargo/bin in PATH)"
echo "  Or run without install: cargo run --bin novad / cargo run --bin novactl -- --help"
echo ""
echo "Next:"
echo "  make dev        # or ./scripts/dev.sh — runs backend + dashboard (http://127.0.0.1:8642 + http://127.0.0.1:5173)"
echo "  make build      # release build"
echo "  make install    # install novad/novactl to ~/.cargo/bin"
echo "  make test       # tests"
echo ""
echo "Verify:  curl http://127.0.0.1:8642/health"
echo "         ./target/debug/novactl runtime status  (or: cargo run --bin novactl -- runtime status)"
echo "Login:   admin / admin123  (auto-created on first run)"
echo "SDK:     NOVA_URL=http://127.0.0.1:8642/api/v1 npx tsx examples/quickstart.ts"
echo ""
