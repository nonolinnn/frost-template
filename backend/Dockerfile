# ------------------------------------------------------------------
# Multi-stage Rust build using cargo-chef for Docker layer caching
# Shared across coordinator and tss-node — select binary via BUILD_BIN arg
# ------------------------------------------------------------------

# ---------- stage 1: chef – install cargo-chef ----------
FROM rust:1.94.0-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

# ---------- stage 2: planner – compute the recipe ----------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---------- stage 3: builder – build deps then app ----------
FROM chef AS builder

# Install system dependencies needed by some crates (e.g. openssl, protobuf)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json

# Build dependencies (cached as long as Cargo.toml / Cargo.lock don't change)
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source and build
COPY . .

ARG BUILD_BIN=coordinator
RUN cargo build --release --bin ${BUILD_BIN}

# Copy built binary to a predictable location
RUN cp /app/target/release/${BUILD_BIN} /app/service

# ---------- stage 4: runtime ----------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r frost && useradd -r -g frost frost
USER frost

COPY --from=builder /app/service /usr/local/bin/service

EXPOSE 8080

CMD ["/usr/local/bin/service"]
