# Nova Runtime Docs

- [Getting Started](getting-started.md) — 2-min `make setup && make dev`, health checks, first SQL/cache/queue calls, admin password handling.
- [Architecture](architecture.md) — one storage engine (BTree+LSM), event bus, execution pipeline, 7+ subsystems.
- [Configuration](configuration.md) — `novad.toml` resolution, minimal vs full, env `NOVA_*`, hot reload `SIGHUP`.
- [API](api.md) — REST `/api/v1/*` + GraphQL `/graphql`, auth `Bearer`, SQL language rules, pagination, errors.
- [CLI](cli.md) — `novactl` reference (`runtime`, `config`, `auth`, `queue`, `scheduler`, `search`, `blob`, `sql`, `db`, `cache`, `completion`, `run`).
- [SDK](sdk.md) — `@novaruntime/sdk` is currently out of sync with the REST API (see the mismatch table); use the raw-fetch pattern from `examples/bloom-market/src/nova.js` for new code.
- [Deployment](deployment.md) — bare metal `systemd`, Docker (`compose`), TLS termination via nginx.
- [Development](development.md) — `Makefile` targets, `cargo test`, `nova-sim`, dashboard `npm`.
- [Compile Report](compile-report.md) — clean `cargo check` + test matrix (123 storage, 45 api, 42 sql, 6 startup).
