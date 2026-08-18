# syntax=docker/dockerfile:1

# ---------------------------------------------------------------------------
# Stage 1: build the frontend (Svelte + Vite -> static files in dist/)
# ---------------------------------------------------------------------------
FROM node:24-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ---------------------------------------------------------------------------
# Stage 2: build the Rust backend
# ---------------------------------------------------------------------------
FROM rust:1.94-slim AS builder
WORKDIR /app
# Dependency layer: only rebuilds deps when Cargo.toml/Cargo.lock change.
# (migrate!() is embedded at compile time, so migrations/ must exist.)
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
RUN mkdir -p src && printf 'fn main() {}\n' > src/main.rs && cargo build --release
COPY src ./src
RUN cargo build --release

# ---------------------------------------------------------------------------
# Stage 3: slim runtime — one binary serving API + static frontend
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/hangar /usr/local/bin/hangar
COPY --from=frontend /app/frontend/dist /app/static
# Reference catalog source files (imported into the DB at startup).
COPY catalog-data /app/catalog-data
ENV PORT=8080 \
    DATA_DIR=/data \
    STATIC_DIR=/app/static \
    CATALOG_DIR=/app/catalog-data
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q --spider http://127.0.0.1:8080/api/health || exit 1
CMD ["hangar"]
