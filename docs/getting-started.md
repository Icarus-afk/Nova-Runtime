# Getting Started — 2 Minutes

This guide gets Nova running, confirms it's healthy, and runs your first SQL, cache, and queue operations. It assumes nothing beyond a terminal.

## Prerequisites

- **Rust 1.85+** (`rustup.rs`) — for `make setup`. Or skip Rust entirely and use [Docker](#docker-no-rust).
- **Node 18+** — only needed for the dashboard and SDK examples.
- **jq** — optional, makes `curl` output readable.

## Fastest Path

```bash
git clone https://github.com/Icarus-afk/Nova-Runtime.git && cd Nova-Runtime
make setup   # 1) checks cargo+node, 2) cargo build --bin novad --bin novactl (debug ~1m), 3) npm install, 4) creates an 8-line novad.toml if missing
make dev     # runs novad (PID, http://127.0.0.1:8642) + vite (http://127.0.0.1:5173), waits up to 30s for /health
```

`make dev` waits for the backend before starting the dashboard, so by the time it prints `✓ backend ready` you can jump straight to the [Verify](#verify) step.

## First login: the admin password

There is **no default `admin/admin123`**. On its very first boot Nova creates the `admin` user and:

1. uses `NOVA_ADMIN_PASSWORD` if you exported it, **or**
2. generates a **random** password and prints it once to the log:

   ```
   Bootstrapped default admin user 'admin'. Password: <random> (set NOVA_ADMIN_PASSWORD to override; change after first login)
   ```

Open the dashboard `http://127.0.0.1:5173` and log in with `admin` plus that password.

> To make demos repeatable, start Nova with a fixed password:
>
> ```bash
> NOVA_ADMIN_PASSWORD="your-password" make dev
> ```
>
> If you later wipe `./data`, the password resets to whatever `NOVA_ADMIN_PASSWORD` is set to (or a new random one). You only have one login attempt with the wrong password every ~12 seconds because login is rate-limited to 5/min per IP.

## Verify

```bash
curl -s http://127.0.0.1:8642/health | jq
# {"status":"healthy","checks":{"storage":true,"memory":true},...}
curl -s http://127.0.0.1:8642/ready | jq
curl -s http://127.0.0.1:8642/metrics | head -n 20   # requires auth (Bearer token)
./target/debug/novactl runtime status
./target/debug/novactl sql query "SELECT 1" --output json
```

The dashboards & SDK:

```bash
# Dashboard
# http://127.0.0.1:5173 → login admin / <password from boot>

# SDK quickstart (raw-fetch — reusable pattern, see docs/sdk.md)
NOVA_USERNAME=admin NOVA_PASSWORD="your-password" npx tsx examples/quickstart.ts
# or with curl only (also needs the password in env):
NOVA_PASSWORD="your-password" ./examples/quickstart.sh
```

## Authenticate from the command line

Login is a public endpoint; everything else needs a Bearer token:

```bash
export NOVA_ADMIN_PASSWORD="your-password"
TOKEN=$(curl -s -X POST http://127.0.0.1:8642/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"'"$NOVA_ADMIN_PASSWORD"'"}' | jq -r .access_token)

# Now authenticated:
curl -s http://127.0.0.1:8642/api/v1/cache/hello -H "Authorization: Bearer $TOKEN"
```

## Your first queries

**SQL**

```bash
curl -s -X POST http://127.0.0.1:8642/api/v1/sql/execute -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"query":"CREATE TABLE demo (id INTEGER PRIMARY KEY, name TEXT)"}'
curl -s -X POST http://127.0.0.1:8642/api/v1/sql/execute -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d "{\"query\":\"INSERT INTO demo (id, name) VALUES (1, 'alice')\"}"
curl -s -X POST http://127.0.0.1:8642/api/v1/sql/query -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"query":"SELECT * FROM demo"}'
```

> Nova's SQL currently uses **bare column names** (no `table.col`) and rejects `CREATE TABLE IF NOT EXISTS` / `DROP TABLE IF EXISTS`. Write plain `CREATE TABLE` / `DROP TABLE <name>`. See the [SQL language notes](../README.md#sql-language-notes).

**Cache**

```bash
curl -s -X POST http://127.0.0.1:8642/api/v1/cache/hello -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"value":{"msg":"world"},"ttl_ms":60000}'
curl -s http://127.0.0.1:8642/api/v1/cache/hello -H "Authorization: Bearer $TOKEN"
```

**Queue**

```bash
curl -s -X POST http://127.0.0.1:8642/api/v1/queues/mydemo -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json'
curl -s -X POST http://127.0.0.1:8642/api/v1/queues/mydemo/messages -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"messages":[{"body":{"hello":"queue"}}]}'
curl -s -X POST http://127.0.0.1:8642/api/v1/queues/mydemo/messages/poll -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"count":1}'
```

## Docker (no Rust)

```bash
docker compose up --build
# backend http://127.0.0.1:8642/health; dashboard at :80 if enabled
curl http://127.0.0.1:8642/health | jq
```

## Config

None required. To customize:

```bash
novactl config default > novad.full.toml  # full commented template
cp novad.full.toml ./novad.toml && $EDITOR novad.toml
# minimal example (what make setup creates):
# [general]
# data_dir = "./data"
# [networking]
# listen_address = "127.0.0.1"
# listen_port = 8642
```

Env overrides: `NOVA_NETWORKING__LISTEN_PORT=8642 RUST_LOG=debug make dev`

Hot reload without restarting: `kill -SIGHUP $(pidof novad)` or `novactl config set logging.level debug`.

## Ports & Paths

- API: `127.0.0.1:8642` → `/health`, `/ready`, `/live`, `/metrics`, `/openapi.json`, `/api/v1/*`, `/graphql`
- Dashboard (dev): `127.0.0.1:5173` (vite proxy → 8642); Docker: `:80` via nginx (optional)
- Data: `./data` + `./data/wal` + `./data/blobs` (see `[general] data_dir`)

## Troubleshooting

- **Port 8642 in use**: `make dev` warns but won't kill the existing `novad`. Kill it with `pkill -f novad` or run a different listen port.
- **Login rejected / 401**: password is the booted one — `NOVA_ADMIN_PASSWORD` or the random value from the log. Not `admin123`.
- **429 too many login attempts**: rate limit is 5/min per IP. Wait the `retry_after_secs` and try again.
- **Build slow**: `make setup` uses the debug profile; `make build` produces a release build (`--release` ~5m).
- **Dashboard blank**: `cd dashboard && npm install && npm run dev` separately; check `5173` is reachable.
- **Auth 401 on `/metrics` or `/graphql`**: both require a Bearer token (unlike `/health`).