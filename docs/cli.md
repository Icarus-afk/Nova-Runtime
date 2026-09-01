# CLI — `novactl`

`novactl` talks to a running `novad` over REST. It is the fastest way to drive every Nova subsystem from a terminal.

Binary: `target/debug/novactl` (built by `make setup`) or `target/release/novactl` (built by `make build`). You can also run it via cargo: `cargo run --bin novactl -- <args>`.

> Note: Nova provides **one** CLI binary, `novactl`. There is no separate `nova` binary in the current build — use `novactl` everywhere.

## Global flags

These are accepted by every subcommand:

| Flag | Env | Purpose |
|------|-----|---------|
| `--config PATH` | — | Path to a `novad.toml` (used by `config` / `runtime` subcommands) |
| `--output table\|json\|yaml` | — | Output format (default `table`) |
| `--address URL` | `NOVA_URL` / `NOVA_API_URL` | API base, default `http://127.0.0.1:8642` |
| `--api-key KEY` | `NOVA_API_KEY` | Sent as `X-API-Key` to authenticate |

```bash
novactl --help                       # top-level help
novactl runtime status               # is novad up?
novactl config show                  # active config
novactl config get storage.page_cache_size
novactl config set logging.level debug
novactl config default > novad.toml  # write full commented template
novactl config validate ./novad.toml
```

## Subcommands

### SQL & DB

```bash
novactl sql query "SELECT * FROM demo WHERE id=\$1" --output json
novactl sql query "SELECT * FROM demo" -f json     # -f / --format (json|csv|arrow)
novactl sql execute "INSERT INTO demo (id, name) VALUES (1, 'alice')"
novactl sql schema demo                            # table schema

novactl db list
novactl db create mydb
novactl db drop mydb
novactl db collections
novactl db create-collection <name>
novactl db drop-collection <name>
novactl db stats
```

### Cache

Get/set/delete individual keys via the REST API (or SDK); the CLI manages the cache as a whole:

```bash
novactl cache list --pattern "session:*"
novactl cache stats
novactl cache clear      # evict all entries
novactl cache flush
```

### Queue

```bash
novactl queue list
novactl queue create myq --durable
novactl queue delete myq
novactl queue publish myq '{"hello":1}'
novactl queue consume myq --count 5
novactl queue stats myq
```

### Scheduler

```bash
novactl scheduler list
novactl scheduler create daily "0 9 * * *" '{"url":"http://worker/run"}'
novactl scheduler pause daily
novactl scheduler resume daily
novactl scheduler delete daily
```

### Search

```bash
novactl search list-indexes
novactl search query "honey" --collection listings_idx --limit 10
novactl search create-index listings_idx <fields...>
novactl search drop-index listings_idx
```

### Blob

```bash
novactl blob list --prefix img/
novactl blob put mykey ./photo.jpg
novactl blob get mykey ./out.jpg
novactl blob delete mykey
```

### Auth & Runtime

```bash
novactl auth list-users
novactl auth create-user alice viewer           # <username> [role]
novactl auth delete-user alice
novactl auth create-api-key "ci-key"
novactl auth revoke-api-key <key-id>

novactl runtime status
novactl runtime start          # start novad
novactl runtime stop           # stop novad
novactl runtime restart
novactl runtime reload         # SIGHUP-style config reload
```

### Misc

```bash
novactl completion bash   # shell completions (bash|zsh|fish|power-shell)
novactl run --data-dir ./data   # run novad directly from the CLI
```

## Authenticating

For most commands `novactl` just needs the API address (default `http://127.0.0.1:8642`). Mutating/admin commands need an API key or token:

```bash
novactl --address http://127.0.0.1:8642 --api-key nr_xxx sql query "SELECT 1" --output json
NOVA_API_KEY=nr_xxx novactl queue publish myq '{"hello":1}'
```

Create an API key from the dashboard or: `curl -X POST http://127.0.0.1:8642/api/v1/auth/api-keys -H "Authorization: Bearer $TOKEN" -d '{"name":"cli"}'`.

## References

- Subcommand definitions: `crates/nova-cli/src/app.rs`
- HTTP client: `crates/nova-cli/src/client.rs`
- Underlying REST API: [api.md](api.md)