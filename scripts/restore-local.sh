#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
[ "${SMASH_ALLOW_RESTORE:-}" = "1" ] || { echo "set SMASH_ALLOW_RESTORE=1 to restore" >&2; exit 1; }
[ -f .env ] || { echo "copy .env.example to .env first" >&2; exit 1; }
backup_dir="${1:?usage: SMASH_ALLOW_RESTORE=1 ./scripts/restore-local.sh BACKUP_DIR}"
set -a
. ./.env
set +a
test -s "$backup_dir/postgres.dump"
docker compose --env-file .env cp "$backup_dir/postgres.dump" postgres:/tmp/smash-restore.dump
docker compose --env-file .env exec -T postgres pg_restore --data-only --disable-triggers --no-owner -U "$POSTGRES_USER" -d "$POSTGRES_DB" /tmp/smash-restore.dump
docker compose --env-file .env run --rm -T -v "$PWD/$backup_dir/objects:/backup:ro" \
  --entrypoint /bin/sh init-minio \
  -c "mc alias set local http://minio:9000 \"$SMASH_MINIO_ROOT_USER\" \"$SMASH_MINIO_ROOT_PASSWORD\" && mc mirror --overwrite /backup local/$SMASH_MINIO_BUCKET"
echo "backup restored from $backup_dir"
