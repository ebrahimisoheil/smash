#!/usr/bin/env bash
set -euo pipefail

# Disposable Community Edition release rehearsal. It builds and starts the
# complete Compose stack, verifies the API, repeats the migration service as an
# upgrade check, and proves restore-based rollback. It never touches the
# default Compose project or its volumes.
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
PROJECT="${ENGRAVE_REHEARSAL_PROJECT:-engrave-phase-j-clean}"
ENV_FILE="${ENGRAVE_ENV_FILE:-.env.example}"
POSTGRES_PORT="${ENGRAVE_REHEARSAL_POSTGRES_PORT:-55436}"
MINIO_PORT="${ENGRAVE_REHEARSAL_MINIO_PORT:-59004}"
MINIO_CONSOLE_PORT="${ENGRAVE_REHEARSAL_MINIO_CONSOLE_PORT:-59005}"
API_PORT="${ENGRAVE_REHEARSAL_API_PORT:-33000}"
BACKUP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/engrave-phase-j-clean.XXXXXX")"

compose() {
  ENGRAVE_POSTGRES_PORT="$POSTGRES_PORT" \
  ENGRAVE_MINIO_PORT="$MINIO_PORT" \
  ENGRAVE_MINIO_CONSOLE_PORT="$MINIO_CONSOLE_PORT" \
  ENGRAVE_API_PORT="$API_PORT" \
    docker compose -p "$PROJECT" --env-file "$ENV_FILE" -f compose.yaml "$@"
}
cleanup() { compose down -v >/dev/null 2>&1 || true; }
trap cleanup EXIT

wait_for_migration() {
  local migration_status=""
  for _ in $(seq 1 60); do
    migration_status="$(compose ps -aq migrate | xargs -r docker inspect -f '{{.State.Status}} {{.State.ExitCode}}' 2>/dev/null || true)"
    case "$migration_status" in
      "exited 0") return 0 ;;
      "exited "*) echo "migration failed: $migration_status" >&2; compose logs migrate; return 1 ;;
    esac
    sleep 2
  done
  echo "migration did not finish: ${migration_status:-no container}" >&2
  return 1
}

command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 1; }
compose build api web
compose up --no-build -d postgres migrate minio init-minio api worker web
wait_for_migration
curl --fail --silent --max-time 10 "http://127.0.0.1:$API_PORT/v1/health" >/dev/null

compose exec -T postgres psql -U "${POSTGRES_USER:-engrave_app}" -d "${POSTGRES_DB:-engrave}" \
  -v ON_ERROR_STOP=1 -c "INSERT INTO tenants (tenant_id,slug,state,created_at,updated_at) VALUES ('00000000-0000-0000-0000-000000000098','clean-install-before','active',now(),now())"
ENGRAVE_ENV_FILE="$ENV_FILE" ENGRAVE_COMPOSE_PROJECT="$PROJECT" \
  ENGRAVE_POSTGRES_PORT="$POSTGRES_PORT" ENGRAVE_MINIO_PORT="$MINIO_PORT" \
  ENGRAVE_MINIO_CONSOLE_PORT="$MINIO_CONSOLE_PORT" bash scripts/backup-local.sh "$BACKUP_DIR"

# Repeat the migration service against the existing volume as the upgrade
# rehearsal, then mutate state and restore the previous release state.
compose up -d migrate
wait_for_migration
compose exec -T postgres psql -U "${POSTGRES_USER:-engrave_app}" -d "${POSTGRES_DB:-engrave}" \
  -v ON_ERROR_STOP=1 -c "UPDATE tenants SET slug='clean-install-after' WHERE tenant_id='00000000-0000-0000-0000-000000000098'"
ENGRAVE_ENV_FILE="$ENV_FILE" ENGRAVE_COMPOSE_PROJECT="$PROJECT" \
  ENGRAVE_POSTGRES_PORT="$POSTGRES_PORT" ENGRAVE_MINIO_PORT="$MINIO_PORT" \
  ENGRAVE_MINIO_CONSOLE_PORT="$MINIO_CONSOLE_PORT" bash scripts/restore-local.sh "$BACKUP_DIR"
compose up -d api worker web >/dev/null
curl --fail --silent --max-time 10 "http://127.0.0.1:$API_PORT/v1/health" >/dev/null
compose exec -T postgres psql -U "${POSTGRES_USER:-engrave_app}" -d "${POSTGRES_DB:-engrave}" -Atc \
  "SELECT slug FROM tenants WHERE tenant_id='00000000-0000-0000-0000-000000000098'" | rg '^clean-install-before$'

{
  printf 'project=%s\n' "$PROJECT"
  printf 'api_health=PASS\n'
  printf 'migration_upgrade=PASS\n'
  printf 'backup_restore_rollback=PASS\n'
  docker inspect "$(compose ps -aq postgres)" --format 'postgres_image={{.Image}}'
  docker inspect "$(compose ps -aq api)" --format 'api_image={{.Image}}'
  docker inspect "$(compose ps -aq web)" --format 'web_image={{.Image}}'
} | tee "$BACKUP_DIR/rehearsal-manifest.txt"
printf 'rehearsal_backup=%s\n' "$BACKUP_DIR"
