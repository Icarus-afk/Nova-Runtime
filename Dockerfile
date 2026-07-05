# syntax=docker/dockerfile:1
FROM rust:1.77-slim-bookworm AS backend-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
RUN cargo build --release --bin novad

FROM node:20-alpine AS dashboard-builder
WORKDIR /app
COPY dashboard/package.json dashboard/package-lock.json ./
RUN npm ci
COPY dashboard/ ./
RUN npm run build

FROM debian:bookworm-slim AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

COPY --from=backend-builder /app/target/release/novad /usr/local/bin/novad
COPY --from=dashboard-builder /app/dist /usr/share/novad/dashboard

EXPOSE 8642

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8642/api/v1/health || exit 1

ENTRYPOINT ["novad"]
