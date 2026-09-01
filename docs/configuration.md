# Configuration

Nova runs with **no configuration file** — the built-in defaults are correct for local development. You only add a `novad.toml` when you want to change ports, paths, resource limits, or production behavior.

## How configuration is resolved

Precedence, highest first:

1. `novad --config PATH` flag
2. `./novad.toml` (in the working directory)
3. `$XDG_CONFIG_HOME/nova/novad.toml`
4. `~/.config/nova/novad.toml`
5. `/etc/novad/novad.toml`
6. Built-in `nova_config::DEFAULT_TOML` (no file needed)

**Environment overrides** use double underscores to separate sections from keys:

```bash
NOVA_NETWORKING__LISTEN_PORT=8642    # [networking] listen_port
NOVA_STORAGE__PAGE_CACHE_SIZE=67108864
```

Setting `NOVA_URL` / `NOVA_API_URL` only matters for the SDK and CLI clients; `novad` itself is configured via `novad.toml` + `NOVA_*` overrides.

## Minimal `novad.toml` (what `make setup` creates)

```toml
[general]
data_dir = "./data"

[networking]
listen_address = "127.0.0.1"
listen_port = 8642

# Optional TLS (mutually required):
# tls_enabled = true
# tls_cert_path = "./certs/cert.pem"
# tls_key_path  = "./certs/key.pem"
```

## Full template

Print every option with its default as commented TOML:

```bash
novactl config default > novad.full.toml
# or: cargo run --bin novactl -- config default
```

## Key sections at a glance

| Section | What it controls |
|---------|------------------|
| `[general]` | `data_dir`, `max_connections`, `shutdown_timeout_ms` |
| `[storage]` | `wal_dir`, `page_cache_size`, `memtable_size`, `fsync_policy` |
| `[memory]` | memory-manager limits |
| `[networking]` | `listen_address`, `listen_port`, TLS (`tls_enabled` + `tls_cert_path` + `tls_key_path`) |
| `[logging]` | `level`, `format` |
| `[cache]` | `max_size`, `default_ttl_secs`, `eviction_policy` |
| `[queue]` | `max_queues`, `enable_scanners`, `scanner_interval_ms` |
| `[blob]` | `chunk_size`, `gc_interval_secs` |
| `[search]` | `default_limit`, `bm25_k1` |
| `[sql]` / `[scheduler]` | engine + job defaults |
| `[event]` | `ordering_shards` (must be a power of two) |
| `[auth]` | `bcrypt_cost`, `lockout`, `mfa`, `password_policy` |
| `[execution]` | `max_concurrent_ops`, `circuit_breaker` |

## Hot reload (no restart)

- **Via API / CLI**: `novactl config get` / `novactl config set logging.level debug` → `PUT /admin/config` merges JSON and validates.
- **Via signal**: `kill -SIGHUP $(pidof novad)` → `ConfigLoader::reload`.

## Validation

`Config::validate()` returns a list of violation strings before starting, e.g. `tls_cert_path must be set when tls_enabled is true`, `ordering_shards must be power of 2`. Boot fails fast if any check fails.

---

See the implementation in `crates/nova-config/src/config.rs` (resolution order, `DEFAULT_TOML` template, reload behavior).