#!/usr/bin/env bash
set -euo pipefail

# Nova Runtime — dev runner (backend + dashboard)
# Usage: ./scripts/dev.sh [--no-dashboard] [--release]

NO_DASHBOARD=0
RELEASE_FLAG=""
for arg in "$@"; do
  case "$arg" in
    --no-dashboard) NO_DASHBOARD=1 ;;
    --release) RELEASE_FLAG="--release" ;;
    --help|-h) echo "Usage: $0 [--no-dashboard] [--release]"; exit 0 ;;
    *) echo "Unknown arg: $arg"; exit 1 ;;
  esac
done

echo "=== Nova Runtime Dev ==="
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

# Auto-setup if binaries missing
BIN="$REPO_DIR/target/debug/novad"
[ -n "$RELEASE_FLAG" ] && BIN="$REPO_DIR/target/release/novad"
if [ ! -f "$BIN" ]; then
    echo "Binary not found at $BIN — running setup..."
    "$SCRIPT_DIR/setup.sh" $RELEASE_FLAG
fi

# Port check (portable: try curl, fallback to ss/lsof)
if curl -s http://127.0.0.1:8642/health >/dev/null 2>&1; then
    echo "⚠  port 8642 already serving (maybe another novad). Continuing anyway."
fi

# Start backend
echo ""
echo "--- Starting novad ($BIN) ---"
cd "$REPO_DIR"
cargo run --bin novad $RELEASE_FLAG -- --log-level debug &
BACKEND_PID=$!
echo "  PID $BACKEND_PID  http://127.0.0.1:8642/health  (admin/admin123)"

# Wait for backend (correct path: /health, not /api/v1/health)
echo "Waiting for backend..."
HEALTH_URL="http://127.0.0.1:8642/health"
for i in $(seq 1 30); do
    if curl -sf "$HEALTH_URL" >/dev/null 2>&1; then
        echo "✓ backend ready ($i sec)"
        break
    fi
    if ! kill -0 $BACKEND_PID 2>/dev/null; then
        echo "✗ backend exited early — check logs above"; exit 1
    fi
    if [ $i -eq 30 ]; then echo "✗ backend not ready after 30s — try: curl $HEALTH_URL"; fi
    sleep 1
done

# Dashboard
FRONTEND_PID=""
if [ $NO_DASHBOARD -eq 0 ] && [ -f "$REPO_DIR/dashboard/package.json" ]; then
    echo ""
    echo "--- Starting dashboard (vite) ---"
    cd "$REPO_DIR/dashboard"
    # ensure deps
    [ -d node_modules ] || npm install >/dev/null 2>&1
    npm run dev &
    FRONTEND_PID=$!
    echo "  PID $FRONTEND_PID  http://127.0.0.1:5173"
else
    echo "(dashboard skipped — use --no-dashboard flag to skip, or install dashboard deps)"
fi

cleanup() {
    echo ""
    echo "Shutting down..."
    [ -n "$FRONTEND_PID" ] && kill $FRONTEND_PID 2>/dev/null || true
    kill $BACKEND_PID 2>/dev/null || true
    wait 2>/dev/null || true
    echo "Done."
}
trap cleanup EXIT INT TERM

echo ""
echo "=== Running ==="
echo "  Backend:   http://127.0.0.1:8642/health  http://127.0.0.1:8642/graphql"
echo "  Dashboard: http://127.0.0.1:5173  (login admin/admin123)"
echo "  Logs:      tail -f data/novad.log (if configured)  or  RUST_LOG=debug cargo run ..."
echo ""
echo "Try:"
echo "  curl http://127.0.0.1:8642/health | jq"
echo "  ./target/debug/novactl sql query \"SELECT 1\"  # or cargo run --bin novactl -- sql query ..."
echo "  NOVA_URL=http://127.0.0.1:8642/api/v1 npx tsx examples/quickstart.ts"
echo ""
echo "Press Ctrl+C to stop."
wait
