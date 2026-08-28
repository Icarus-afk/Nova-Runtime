# Development

## Make Targets

```
make help         Show help
make setup        Checks cargo+node, debug builds novad+novactl, npm install, creates novad.toml
make dev          Runs novad (8642) + vite (5173) with health-wait on /health, Ctrl+C cleanup
make build        cargo build --release --bin novad --bin novactl
make test         cargo test --workspace --exclude nova-sim + sdk/dashboard builds
make docker       docker compose up --build
make fmt/lint/check/clean
```

`scripts/setup.sh [--release|--config-only]` / `scripts/dev.sh [--no-dashboard|--release]` are the same as `make`.

## Tests

- `cargo test -p nova-storage --lib` 122 ok (LSM bloom `wrapping_mul` fix)
- `cargo test -p nova-api --lib` 34 ok, `--test startup_shutdown` 6 ok (health `checks` alias)
- `cargo test --workspace --exclude nova-sim` ~1,500 total (1 flaky `budget_zero_max` in `nova-sim` not in workspace)

## Sim & Bench

```bash
cargo run -p nova-sim -- --headless --ticks 10000 --output results.json
jq .summary results.json
```

## Dashboard

```bash
cd dashboard && npm install && npm run dev   # 5173 proxy → 8642
npm run build  # tsc + vite build → dist/
```

API client `dashboard/src/api/client.ts`: `request()` now on `401` clears token → `/login`; mutations (`publishMessage`, `triggerJob`, etc.) throw instead of mock `id:''`, reads still degrade to `[]` but log warn.

## SDK

`cd sdk && npm run build` → `dist/`. Use `NOVA_URL=http://127.0.0.1:8642/api/v1` via `fromEnv()`.

## Adding a Subsystem

1. New crate `crates/nova-xxx` with `Manager` + `StorageEngine` adapter + `Config`.
2. Expose via `nova-api/src/routes/xxx.rs` + `nova-config` section.
3. Wire in `novad/src/main.rs` (init, event bus, shutdown).
4. Add `novactl` command + `sdk/src/xxx.ts` + dashboard page.

## Style

`cargo fmt --all`, `cargo clippy --workspace -- -D warnings` (199 warnings → 27 after fixes, remaining `dead_code` intentional).

See `crates/*/src/*.rs` and `BUILD_REPORT.md`/`docs/compile-report.md`.
