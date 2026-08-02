# Phase B8 gate audit

The B8 evidence bundle is reproducible from a clean checkout:

```sh
cargo fmt --all -- --check
cargo build --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
./scripts/check-openapi.sh
docker compose --env-file .env.example config
```

The persistence proof is:

```sh
cp .env.example .env
docker compose --env-file .env up -d --build api worker
curl -fsS http://localhost:3000/v1/readiness
./scripts/seed-sales.sh
./scripts/seed-sales.sh
SMASH_ALLOW_DESTRUCTIVE_RESTORE=1 \
  ./scripts/prove-backup-restore.sh var/backups/b8-proof
```

The proof covers migration-gated startup, non-mutating readiness, deterministic
fixture state, idempotent seeding, tenant-scoped object keys, PostgreSQL data,
append-only Events, and MinIO object recovery. CI remains the evidence source
for the dependency-policy, core-boundary, and OpenAPI jobs.
