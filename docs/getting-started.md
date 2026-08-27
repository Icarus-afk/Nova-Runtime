# Getting Started — 2 Minutes

## Prerequisites

- **Rust 1.85+** (`https://rustup.rs`) or use Docker (no Rust)
- **Node 18+** for dashboard/SDK (optional)
- `jq` optional for pretty `curl` output

## Fastest Path

```bash
git clone https://github.com/Icarus-afk/Nova-Runtime.git && cd Nova-Runtime
make setup   # 1) checks cargo+node, 2) cargo build --bin novad --bin novactl (debug ~1m), 3) npm install, 4) creates 8-line novad.toml if missing
make dev     # runs novad (PID, http://127.0.0.1:8642) + vite (http://127.0.0.1:5173) with 30s health-wait on /health
```

Open dashboard: `http://127.0.0.1:5173` → login `admin` / `admin123` (auto-created on first run).

## Verify

```bash
curl -s http://127.0.0.1:8642/health | jq
# {"status":"healthy","checks":{"storage":true,"memory":true},...}
curl -s http://127.0.0.1:8642/ready | jq
curl -s http://127.0.0.1:8642/metrics | head -n 20
./target/debug/novactl runtime status
./target/debug/novactl sql query "SELECT 1" --output json
```

SDK:

```bash
NOVA_URL=http://127.0.0.1:8642/api/v1 npx tsx examples/quickstart.ts
# or curl:
./examples/quickstart.sh
```

## Config

None required. To customize:

```bash
novactl config default > novad.full.toml  # full commented template (embedded DEFAULT_TOML)
cp novad.full.toml ./novad.toml && $EDITOR novad.toml
# minimal example (what make setup creates):
# [general]
# data_dir = "./data"
# [networking]
# listen_address = "127.0.0.1"
# listen_port = 8642
```

Env overrides: `NOVA_NETWORKING__LISTEN_PORT=8642 RUST_LOG=debug make dev`

Hot reload: `kill -SIGHUP $(pidof novad)` or `novactl config set logging.level debug`

## Ports & Paths

- API: `127.0.0.1:8642` → `/health`, `/ready`, `/live`, `/metrics`, `/openapi.json`, `/api/v1/*`, `/graphql`
- Dashboard (dev): `127.0.0.1:5173` (vite proxy → 8642); Docker: `:80` via nginx (optional)
- Data: `./data` + `./data/wal` + `./data/blobs` (see `[general] data_dir`)

## Troubleshooting

- **Port 8642 in use**: `make dev` warns; kill old `novad` or change `novad.toml` `listen_port`.
- **Build slow**: `make setup` uses debug; `make build` for release (`--release` ~5m).
- **Dashboard blank**: `cd dashboard && npm install && npm run dev` separately; check `5173` not firewalled.
- **Auth 401**: dashboard auto-redirects to `/login`; CLI needs `--api-key` or `NOVA_API_KEY`.
