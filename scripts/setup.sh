#!/usr/bin/env bash
set -euo pipefail

echo "=== Nova Runtime Setup ==="

# Check prerequisites
command -v cargo >/dev/null 2>&1 || { echo "Error: Rust/Cargo not found. Install from https://rustup.rs"; exit 1; }
command -v node >/dev/null 2>&1 || { echo "Error: Node.js not found. Install from https://nodejs.org"; exit 1; }
command -v npm >/dev/null 2>&1 || { echo "Error: npm not found."; exit 1; }

echo "✓ prerequisites found"

# Build backend
echo ""
echo "--- Building backend (cargo build) ---"
cargo build --release 2>&1 | tail -5
echo "✓ backend build complete"

# Install dashboard dependencies
echo ""
echo "--- Installing dashboard dependencies ---"
cd "$(dirname "$0")/../dashboard"
npm install 2>&1 | tail -3
echo "✓ dashboard dependencies installed"

# Create default config in repo root if not exists
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
if [ ! -f "$ROOT_DIR/novad.toml" ]; then
    echo ""
    echo "--- Creating default novad.toml ---"
    cat > "$ROOT_DIR/novad.toml" << 'TOML'
[general]
data_dir = "./data"

[networking]
listen_address = "127.0.0.1"
listen_port = 8642
tls_enabled = false

[storage]
wal_dir = "./data/wal"
page_cache_size = 65536
memtable_size = 67108864
fsync_policy = { every_n_ms = 100 }

[cache]
max_size = 268435456
default_ttl_secs = 3600
eviction_policy = "Lru"
backend_type = "HashMap"

[auth.session]
ttl_seconds = 86400
max_active_sessions = 100
cache_size = 100000

[auth.internal]
bcrypt_cost = 12
enable_brute_force_detection = true

[auth.internal.lockout]
max_attempts = 5
duration_secs = 900

[auth.internal.mfa]
issuer = "Nova Runtime"
window = 1

[auth.internal.password_policy]
min_length = 8
max_length = 128
min_lowercase = 1
min_uppercase = 1
min_digits = 1
min_special = 0

[execution]
max_concurrent_ops = 100
pipeline_queue_depth = 10000
worker_threads = 4
default_operation_timeout_ms = 30000

[blob]
chunk_size = 4194304
max_blob_size = 1073741824
gc_interval_secs = 3600
gc_grace_period_secs = 86400
data_dir = "./data/blobs"
chunk_nesting_depth = 2

[search]
default_limit = 20
max_limit = 100

[sql]
max_batch_size = 100
max_columns = 100

[queue]
max_queues = 100
max_messages_per_queue = 10000
max_message_size = 262144
default_visibility_timeout_secs = 30
message_ttl_secs = 604800
enable_scanners = true

[scheduler]
time_wheel_tick_ms = 100
time_wheel_slots = 360
max_concurrent_jobs = 10
enable_startup_recovery = true

[event]
ordering_shards = 4
dlq_max_entries = 100
TOML
    echo "✓ default config created"
fi

echo ""
echo "=== Setup complete ==="
echo ""
echo "Start the backend:  cargo run --bin novad"
echo "Start the dashboard: cd dashboard && npm run dev"
echo "Default login:      admin / admin123"
