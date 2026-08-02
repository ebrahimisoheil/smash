# Phase B local stack runbook

## Prerequisites

- Docker Engine with Compose v2.
- Rust toolchain pinned by `rust-toolchain.toml` for local builds.
- At least 8 GB memory available to the build and database containers.

## Start and stop

```sh
cp .env.example .env
docker compose up --build
docker compose down
```

The Compose topology orders PostgreSQL health before the one-shot migration,
and MinIO health before idempotent bucket initialization. API and worker wait
for both boundaries to complete, so repeating `docker compose up` is safe.

API is at `http://localhost:${SMASH_API_PORT:-3000}`, web at
`http://localhost:${SMASH_WEB_PORT:-3001}`, MinIO API at
`http://localhost:${SMASH_MINIO_PORT:-9000}`, and the MinIO console at
`http://localhost:${SMASH_MINIO_CONSOLE_PORT:-9001}`.

API and worker start only after PostgreSQL/MinIO health checks and the one-shot
migration job succeed. A restart with `docker compose up` preserves named
volumes and must not duplicate initialization.

## Reset versus restore

`./scripts/reset-local.sh` is a destructive local reset. It removes PostgreSQL
and MinIO named volumes and cannot substitute for restore evidence.

Backup and restore commands are intentionally explicit:

```sh
./scripts/backup-local.sh ./var/backups/latest
./scripts/restore-local.sh ./var/backups/latest
```

The scripts require a running stack, record the fixture/stack version, and keep
database and object backups under one timestamped directory.
