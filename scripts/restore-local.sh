#!/usr/bin/env bash
set -euo pipefail

# Restore the paired local backup produced by backup-local.sh. The database
# schema is replaced, so this command is deliberately explicit and destructive
# to the current local Compose data.
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
BACKUP_DIR="${1:?usage: $0 BACKUP_DIR}"
ENV_FILE="${ENGRAVE_ENV_FILE:-.env}"
PROJECT="${ENGRAVE_COMPOSE_PROJECT:-engrave-v2}"
OBJECT_BACKUP_IMAGE="${ENGRAVE_BACKUP_IMAGE:-alpine:3.20}"
EXPORT_FORMAT="engrave-community-recovery-v1"
for file in FORMAT postgres.dump minio-data.tar SHA256SUMS; do
  test -f "$BACKUP_DIR/$file" || { echo "missing $BACKUP_DIR/$file" >&2; exit 1; }
done
test "$(tr -d '\r\n' < "$BACKUP_DIR/FORMAT")" = "$EXPORT_FORMAT" || {
  echo "unsupported recovery export format" >&2
  exit 1
}
check_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum --check "$1"; else shasum -a 256 -c "$1"; fi
}
(cd "$BACKUP_DIR" && check_sha256 SHA256SUMS)

compose() { docker compose -p "$PROJECT" --env-file "$ENV_FILE" -f compose.yaml "$@"; }
compose stop api worker web >/dev/null 2>&1 || true
compose exec -T postgres psql -U "${POSTGRES_USER:-engrave_app}" -d "${POSTGRES_DB:-engrave}" \
  -v ON_ERROR_STOP=1 -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;'
cat "$BACKUP_DIR/postgres.dump" | compose exec -T postgres pg_restore --no-owner --no-privileges \
  --username="${POSTGRES_USER:-engrave_app}" --dbname="${POSTGRES_DB:-engrave}"
MINIO_CONTAINER="$(compose ps -q minio)"
test -n "$MINIO_CONTAINER" || { echo "MinIO container is not running" >&2; exit 1; }
docker run --rm --volumes-from "$MINIO_CONTAINER" "$OBJECT_BACKUP_IMAGE" sh -c 'find /data -mindepth 1 -maxdepth 1 -exec rm -rf {} +'
cat "$BACKUP_DIR/minio-data.tar" | docker run --rm -i --volumes-from "$MINIO_CONTAINER" "$OBJECT_BACKUP_IMAGE" tar -C /data -xf -
echo "Restore completed from $BACKUP_DIR; start Compose and run migrations before serving traffic."
