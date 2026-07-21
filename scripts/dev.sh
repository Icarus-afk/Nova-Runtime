#!/usr/bin/env bash
set -euo pipefail

echo "=== Nova Runtime Dev Server ==="

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

# Ensure setup has been run
if [ ! -f "$REPO_DIR/target/release/novad" ] && [ ! -f "$REPO_DIR/target/debug/novad" ]; then
    echo "First run detected. Running setup..."
    "$SCRIPT_DIR/setup.sh"
fi

# Check if port is already in use
if ss -tlnp | grep -q ":8642 "; then
    echo "Warning: port 8642 appears to be in use"
fi

# Start backend in background
echo ""
echo "--- Starting novad backend ---"
cd "$REPO_DIR"
cargo run --bin novad &
BACKEND_PID=$!
echo "novad PID: $BACKEND_PID"

# Wait for backend to be ready
echo ""
echo "Waiting for backend..."
for i in $(seq 1 30); do
    if curl -s http://127.0.0.1:8642/api/v1/health > /dev/null 2>&1; then
        echo "✓ backend ready"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "Warning: backend not responding after 30s"
    fi
    sleep 1
done

# Start dashboard
echo ""
echo "--- Starting dashboard dev server ---"
cd "$REPO_DIR/dashboard"
npm run dev &
FRONTEND_PID=$!
echo "Vite PID: $FRONTEND_PID"

# Trap to clean up on exit
cleanup() {
    echo ""
    echo "Shutting down..."
    kill $FRONTEND_PID 2>/dev/null || true
    kill $BACKEND_PID 2>/dev/null || true
    wait
    echo "Done."
}
trap cleanup EXIT INT TERM

echo ""
echo "=== Nova Runtime running ==="
echo "  Backend:  http://127.0.0.1:8642"
echo "  Dashboard: http://127.0.0.1:5173"
echo "  Login:     admin / admin123"
echo ""
echo "Press Ctrl+C to stop."

wait
