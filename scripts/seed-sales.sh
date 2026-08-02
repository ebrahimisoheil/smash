#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
[ -f .env ] || { echo "copy .env.example to .env first" >&2; exit 1; }
set -a
. ./.env
set +a

docker compose --env-file .env exec -T postgres psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" < eval/fixtures/sales/seed.sql
docker compose --env-file .env run --rm -T \
  -v "$PWD/eval/fixtures/sales:/fixture:ro" \
  --entrypoint /bin/sh init-minio \
  -c "mc alias set local http://minio:9000 \"$SMASH_MINIO_ROOT_USER\" \"$SMASH_MINIO_ROOT_PASSWORD\" && \
      mc cp /fixture/assets/discovery-call.vtt local/$SMASH_MINIO_BUCKET/tenants/018f0000-0000-7000-8000-000000000001/sources/018f0000-0000-7000-8000-000000000200/versions/018f0000-0000-7000-8000-000000000201 && \
      mc cp /fixture/assets/quarterly-review.pdf local/$SMASH_MINIO_BUCKET/tenants/018f0000-0000-7000-8000-000000000001/sources/018f0000-0000-7000-8000-000000000210/versions/018f0000-0000-7000-8000-000000000212"
echo "sales fixture seeded idempotently"
