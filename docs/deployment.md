# Deployment

## Bare Metal (systemd)

```bash
cargo build --release --bin novad --bin novactl
sudo cp target/release/novad target/release/novactl /usr/local/bin/
sudo mkdir -p /var/lib/novad /etc/novad
novactl config default | sudo tee /etc/novad/novad.toml

cat | sudo tee /etc/systemd/system/novad.service <<'EOF'
[Unit]
Description=Nova Runtime
After=network.target
[Service]
User=novad
ExecStart=/usr/local/bin/novad --config /etc/novad/novad.toml
Restart=always
LimitNOFILE=4096
[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload && sudo systemctl enable --now novad
curl http://127.0.0.1:8642/health
```

## Docker

```bash
docker build -t nova-runtime . # rust:1.85 builder + node 20 dashboard + debian slim runtime, copies novad+novactl+dist
docker run -d --name nova-runtime -p 8642:8642 -v nova_data:/var/lib/novad nova-runtime
# or:
docker compose up --build # uses env_file .env, novad.toml mount optional (defaults)
```

`docker-compose.yml` healthchecks `http://localhost:8642/health`. No `novad.toml` required — image uses built-in defaults.

## TLS

`novad` validates `[networking] tls_enabled` requires `tls_cert_path`+`tls_key_path` (else `cargo test` fails). Terminate TLS at nginx: example `docker/nginx.conf` proxies `8642` → `80/443`. For native TLS, front with `stunnel` or add `axum-server` rustls (planned).

## Observability

- Probes: `GET /health` (checks storage+memory + `subsystems`), `/ready`, `/live`, `/metrics` (Prometheus `nova_uptime_secs`, `nova_operations_total`, etc.), `GET /admin/status` pipeline metrics.
- Logs: JSON to stdout + optional file `logging.file`; `RUST_LOG=debug` or `novactl config set`.
- Graceful shutdown: `systemd` `SIGTERM` → drains pipeline 30s, `store.close()`, subsystems `shutdown()` (cache event abort, search flag, sql flag, blob `save_dedup`, scheduler flag, queue scanner via `watch`).

## Data

`data/` (default `./data`): `wal/`, `blobs/`, `sql:table:*` keys in storage. Backup: snapshot `data/` while `novad` stopped or use `store` WAL segment `fsync`.

See `docs/configuration.md` for the full `novad.toml` reference and `crates/novad/src/main.rs`.
