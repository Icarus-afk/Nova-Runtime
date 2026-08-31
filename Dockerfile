# syntax=docker/dockerfile:1
# Pinned digests for supply chain — update via `docker build --pull` and `docker images --digests`
FROM rust:1.85-slim-bookworm AS backend-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && cp /app/target/release/novad /tmp/novad && cp /app/target/release/novactl /tmp/novactl

FROM node:20-alpine AS dashboard-builder
WORKDIR /app
COPY dashboard/package.json dashboard/package-lock.json ./
RUN npm ci
COPY dashboard/ ./
RUN npm run build

FROM debian:bookworm-slim AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl tini && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd -r novad && useradd -r -g novad -d /data -m novad && \
    mkdir -p /data /var/lib/novad && chown novad:novad /data /var/lib/novad

WORKDIR /app
ENV NOVA_GENERAL__DATA_DIR=/data
ENV NOVA_STORAGE__WAL_DIR=/data/wal
ENV NOVA_BLOB__DATA_DIR=/data/blobs
ENV RUST_LOG=info

COPY --from=backend-builder /tmp/novad /usr/local/bin/novad
COPY --from=backend-builder /tmp/novactl /usr/local/bin/novactl
COPY --from=dashboard-builder /app/dist /usr/share/novad/dashboard

EXPOSE 8642
USER novad
HEALTHCHECK --interval=30s --timeout=3s --start-period=15s --retries=3 \
    CMD curl -f http://localhost:8642/health || exit 1

ENTRYPOINT ["tini", "--", "novad"]
