# Nova Runtime Docs

- [Getting Started](getting-started.md) — 2-min `make setup && make dev`, health checks, first SQL/cache/queue calls.
- [Architecture](architecture.md) — single storage (BTree+LSM), event bus, execution pipeline, 7 subsystems.
- [Configuration](configuration.md) — `novad.toml` resolution, minimal vs full, env `NOVA_*`, hot reload `SIGHUP`.
- [API](api.md) — REST ` /api/v1/*` + GraphQL `/graphql`, auth `Bearer`, pagination `limit/offset`, errors.
- [CLI](cli.md) — `novactl` (`runtime`, `config`, `sql`, `cache`, `queue`, `scheduler`, `search`, `blob`, `auth`).
- [SDK](sdk.md) — `@novaruntime/sdk` (`createClient`/`fromEnv`), `NOVA_URL`, quickstart.ts/sh.
- [Deployment](deployment.md) — bare metal `systemd`, Docker (`compose`), TLS termination via nginx.
- [Development](development.md) — `Makefile` targets, `cargo test`, `nova-sim`, dashboard `npm`.
- [Compile Report](compile-report.md) — clean `cargo check` + test matrix (122 storage, 34 api, 6 startup).
