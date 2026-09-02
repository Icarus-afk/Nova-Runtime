# Development

## Make Targets

```
make help         Show help
make setup        Checks cargo+node, debug builds novad+novactl, npm install, creates novad.toml
make dev          Runs novad (8642) + vite (5173) with health-wait on /health, Ctrl+C cleanup
make build        cargo build --release --bin novad --bin novactl
make test         cargo test --workspace + sdk/dashboard builds
make docker       docker compose up --build
make fmt/lint/check/clean
```

`scripts/setup.sh [--release|--config-only]` / `scripts/dev.sh [--no-dashboard|--release]` are the same as `make`.

## Tests

```bash
cargo test -p nova-storage --lib          # 123 ok (LSM bloom, BTree, WAL)
cargo test -p nova-api --lib              # 45 ok (REST handlers, middleware, auth)
cargo test -p nova-api --test startup_shutdown   # 6 ok (health `checks` alias, clean stop)
cargo test -p nova-sql --test sql_integration    # 42 ok (JOIN, GROUP BY/HAVING, ORDER BY, unify_types)
cargo test --workspace        # everything above plus SDK/dashboard build
```

## Dashboard

```bash
cd dashboard && npm install && npm run dev   # 5173 proxy → 8642
npm run build  # tsc + vite build → dist/
```

API client `dashboard/src/api/client.ts`: `request()` on `401` clears token → `/login`; reads degrade to `[]` but log warn.

## SDK

`cd sdk && npm run build` → `dist/`. The SDK currently targets an older API surface — for new clients use the raw-fetch pattern in `examples/bloom-market/src/nova.js` (see `docs/sdk.md`). Dashboard api client (`dashboard/src/api/client.ts`) is the reference for current routes.

## Adding a Subsystem

1. New crate `crates/nova-xxx` with `Manager` + `StorageEngine` adapter + `Config`.
2. Expose via `nova-api/src/routes/xxx.rs` + `nova-config` section.
3. Wire in `novad/src/main.rs` (init, event bus, shutdown).
4. Add `novactl` command + `sdk/src/xxx.ts` + dashboard page.

## Style

`cargo fmt --all`, `cargo clippy --workspace -- -D warnings` (clean — 0 warnings).

See `crates/*/src/*.rs` and `BUILD_REPORT.md`/`docs/compile-report.md`.
