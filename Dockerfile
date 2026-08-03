# syntax=docker/dockerfile:1
#
# Multi-stage build for the ENGRAVE V2 Rust workspace: one dependency-cached
# build produces both the `api` and `worker` binaries (`engrave-api`,
# `engrave-worker`), copied into a slim runtime image.
#
# Phase A note: this Dockerfile builds the current skeleton crates (stub
# main.rs bodies, no real routes/job loop). It exists to prove the build
# and image shape now, not to describe a production deployment — no
# Compose file, no schema, no secrets wiring. That's Phase B+.
#
# Build (from repo root):
#   docker build -f Dockerfile -t engrave-v2:dev .

# Pinned by digest (multi-arch index digest for the `1.97.1-slim-bookworm`
# tag, resolved via `docker buildx imagetools inspect
# rust:1.97.1-slim-bookworm` at scaffold time) rather than floating on
# `rust:slim` — floating base images are a supply chain footgun the
# roadmap's stack ratification (ADR-0001) explicitly wants to avoid at the
# infrastructure layer too. Re-pin periodically (Renovate/Dependabot can
# automate this once wired up); the tag is kept in the image reference
# purely as a human-readable label — the digest is what's actually pulled.
FROM rust:1.97.1-slim-bookworm@sha256:99e09cb2284e2ddbb73a995deee3e91783fd04d177602ccf6eab326d778ee777 AS chef
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    protobuf-compiler \
    libprotobuf-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked

# ---------------------------------------------------------------------------
# Planner: compute a dependency recipe from the manifests only, so the
# expensive dependency-compile layer below only invalidates when a
# Cargo.toml/Cargo.lock actually changes — not on every source edit.
# ---------------------------------------------------------------------------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---------------------------------------------------------------------------
# Builder: compile dependencies from the cached recipe, then the workspace.
# ---------------------------------------------------------------------------
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Builds and caches every workspace dependency — this layer is reused across
# builds as long as Cargo.toml/Cargo.lock don't change.
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin api --bin worker

# ---------------------------------------------------------------------------
# Runtime: slim Debian base, no toolchain, just the two binaries.
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818 AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 engrave

WORKDIR /app
COPY --from=builder /app/target/release/api ./api
COPY --from=builder /app/target/release/worker ./worker

USER engrave
ENV RUST_LOG=info

# The image ships both binaries; the entrypoint (and therefore whether this
# container runs the API or the worker) is selected at `docker run` /
# Compose service definition time, not baked in here — that's how "one
# image, two entrypoints" from 06-service-architecture.md §6.2 is meant to
# work. Compose wiring itself is out of scope for Phase A.
ENTRYPOINT ["/app/api"]
