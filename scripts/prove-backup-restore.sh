#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
[ "${SMASH_ALLOW_DESTRUCTIVE_RESTORE:-}" = "1" ] || { echo "set SMASH_ALLOW_DESTRUCTIVE_RESTORE=1 to run the clean restore proof" >&2; exit 1; }
proof_dir="${1:-var/backups/proof-$(date -u +%Y%m%dT%H%M%SZ)}"
./scripts/seed-sales.sh
./scripts/backup-local.sh "$proof_dir"
docker compose --env-file .env down -v
docker compose --env-file .env up -d postgres minio
docker compose --env-file .env run --rm migrate
docker compose --env-file .env run --rm init-minio
SMASH_ALLOW_RESTORE=1 ./scripts/restore-local.sh "$proof_dir"
set -a
. ./.env
set +a
count="$(docker compose --env-file .env exec -T postgres psql --set ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Atqc "select count(*) from events where tenant_id='018f0000-0000-7000-8000-000000000001'")"
[ "$count" = "3" ] || { echo "expected 3 fixture events, got $count" >&2; exit 1; }
echo "backup/restore proof passed: fixture metadata, events, and object backup restored"
