#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
[ -f .env ] || { echo "copy .env.example to .env first" >&2; exit 1; }
backup_dir="${1:?usage: ./scripts/backup-local.sh BACKUP_DIR}"
set -a
. ./.env
set +a
mkdir -p "$backup_dir/objects"
docker compose --env-file .env exec -T postgres pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --format=custom --file=/tmp/smash-backup.dump
docker compose --env-file .env cp postgres:/tmp/smash-backup.dump "$backup_dir/postgres.dump"
docker compose --env-file .env run --rm -T -v "$PWD/$backup_dir/objects:/backup" \
  --entrypoint /bin/sh init-minio \
  -c "mc alias set local http://minio:9000 \"$SMASH_MINIO_ROOT_USER\" \"$SMASH_MINIO_ROOT_PASSWORD\" && mc mirror local/$SMASH_MINIO_BUCKET /backup"
cp eval/fixtures/sales/fixture.toml "$backup_dir/fixture.toml"
date -u +%Y-%m-%dT%H:%M:%SZ > "$backup_dir/created-at.txt"
echo "backup written to $backup_dir"
