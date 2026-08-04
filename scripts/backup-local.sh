#!/usr/bin/env bash
set -euo pipefail

# Back up the canonical PostgreSQL database and MinIO object store together.
# This is intentionally an operator-invoked local/Compose proof, not a
# production retention or encryption policy.
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
BACKUP_DIR="${1:?usage: $0 BACKUP_DIR}"
ENV_FILE="${ENGRAVE_ENV_FILE:-.env}"
PROJECT="${ENGRAVE_COMPOSE_PROJECT:-engrave-v2}"
OBJECT_BACKUP_IMAGE="${ENGRAVE_BACKUP_IMAGE:-alpine:3.20}"
EXPORT_FORMAT="engrave-community-recovery-v1"
mkdir -p "$BACKUP_DIR"

printf '%s\n' "$EXPORT_FORMAT" > "$BACKUP_DIR/FORMAT"

compose() { docker compose -p "$PROJECT" --env-file "$ENV_FILE" -f compose.yaml "$@"; }
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

compose exec -T postgres pg_dump --format=custom --no-owner --no-privileges \
  --username="${POSTGRES_USER:-engrave_app}" --dbname="${POSTGRES_DB:-engrave}" \
  > "$BACKUP_DIR/postgres.dump"
MINIO_CONTAINER="$(compose ps -q minio)"
test -n "$MINIO_CONTAINER" || { echo "MinIO container is not running" >&2; exit 1; }
docker run --rm --volumes-from "$MINIO_CONTAINER" "$OBJECT_BACKUP_IMAGE" tar -C /data -cf - . > "$BACKUP_DIR/minio-data.tar"
sha256 "$BACKUP_DIR/FORMAT" "$BACKUP_DIR/postgres.dump" "$BACKUP_DIR/minio-data.tar" > "$BACKUP_DIR/SHA256SUMS"
{
  printf 'export_format=%s\n' "$EXPORT_FORMAT"
  printf 'compose_project=%s\n' "$PROJECT"
  printf 'created_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'git_revision=%s\n' "$(git rev-parse HEAD 2>/dev/null || printf unknown)"
  printf 'schema_migrations=\n'
  compose exec -T postgres psql -AtX -U "${POSTGRES_USER:-engrave_app}" -d "${POSTGRES_DB:-engrave}" \
    -c 'SELECT version FROM _sqlx_migrations ORDER BY version' 2>/dev/null || true
} > "$BACKUP_DIR/manifest.txt"
echo "Backup written to $BACKUP_DIR"
