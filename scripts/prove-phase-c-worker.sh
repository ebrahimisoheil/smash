#!/usr/bin/env bash
set -euo pipefail

# Requires the local Compose PostgreSQL service and the Phase C migration.
# The proof uses one tenant-scoped text operation, then runs the real worker
# binary and checks durable operation, source, artifact, chunk, processor-run,
# and process-evidence records. It never writes Memory.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ENV_FILE="${ENGRAVE_ENV_FILE:-.env.example}"
CARGO_BIN="${CARGO_BIN:-cargo}"
if ! command -v "$CARGO_BIN" >/dev/null 2>&1 && [[ -x "${HOME:-}/.cargo/bin/cargo" ]]; then
  CARGO_BIN="${HOME}/.cargo/bin/cargo"
fi
command -v "$CARGO_BIN" >/dev/null 2>&1 || {
  echo "cargo is required; set CARGO_BIN to the pinned toolchain binary" >&2
  exit 1
}
TENANT_ID="00000000-0000-0000-0000-000000000001"
AREA_ID="00000000-0000-0000-0000-000000000010"
SOURCE_ID="00000000-0000-0000-0000-000000000020"
VERSION_ID="00000000-0000-0000-0000-000000000021"
OPERATION_ID="00000000-0000-0000-0000-000000000030"

docker compose --env-file "$ENV_FILE" -f compose.yaml exec -T postgres psql -U engrave_app -d engrave -v ON_ERROR_STOP=1 <<SQL
SET app.tenant_id = '$TENANT_ID';
INSERT INTO tenants (tenant_id, slug, state, created_at, updated_at)
VALUES ('$TENANT_ID', 'phase-c-proof', 'active', now(), now()) ON CONFLICT DO NOTHING;
INSERT INTO areas (area_id, tenant_id, slug, state, version)
VALUES ('$AREA_ID', '$TENANT_ID', 'proof', 'active', 1) ON CONFLICT DO NOTHING;
INSERT INTO sources (source_id, tenant_id, area_id, state, title, version, created_at, updated_at)
VALUES ('$SOURCE_ID', '$TENANT_ID', '$AREA_ID', 'queued', 'Phase C proof', 1, now(), now())
ON CONFLICT (source_id) DO UPDATE SET state = 'queued', updated_at = now();
INSERT INTO source_versions (source_version_id, tenant_id, source_id, version_number, state, object_key, media_type, byte_size, checksum, created_at)
VALUES ('$VERSION_ID', '$TENANT_ID', '$SOURCE_ID', 1, 'current', 'tenants/$TENANT_ID/sources/$SOURCE_ID/versions/$VERSION_ID', 'text/markdown', 34, 'phase-c-proof', now()) ON CONFLICT DO NOTHING;
UPDATE source_versions SET state = 'current', quarantine_reason = NULL WHERE source_version_id = '$VERSION_ID';
INSERT INTO operations (operation_id, tenant_id, state, payload, idempotency_scope, idempotency_key, created_at, updated_at)
VALUES ('$OPERATION_ID', '$TENANT_ID', 'queued', '{"source_id":"$SOURCE_ID","source_version_id":"$VERSION_ID","content":"# Proof\\nAlpha evidence\\nBeta evidence\\n","media_type":"text/markdown","processor_name":"engrave-text","processor_version":"1"}'::jsonb, 'phase-c-proof', 'phase-c-proof-1', now(), now())
ON CONFLICT (operation_id) DO UPDATE SET state = 'queued', attempt = 0, lease_token = NULL, lease_expires_at = NULL, cancel_requested = false, error_code = NULL, error_message = NULL, updated_at = now();
SQL

ENGRAVE_DATABASE_URL="postgres://engrave_app:engrave_local_only_change_me@127.0.0.1:5432/engrave" \
ENGRAVE_TENANT_ID="$TENANT_ID" \
timeout "${ENGRAVE_WORKER_TIMEOUT:-15}" \
"$CARGO_BIN" run --manifest-path Cargo.toml -p engrave-worker >/tmp/engrave-phase-c-worker.log 2>&1 || test "$?" -eq 124

docker compose --env-file "$ENV_FILE" -f compose.yaml exec -T postgres psql -U engrave_app -d engrave -Atc "
SET app.tenant_id = '$TENANT_ID';
SELECT CASE WHEN (SELECT state FROM operations WHERE operation_id = '$OPERATION_ID') = 'succeeded'
 AND (SELECT state FROM sources WHERE source_id = '$SOURCE_ID') = 'ready'
 AND (SELECT count(*) FROM artifacts WHERE source_version_id = '$VERSION_ID') >= 1
 AND (SELECT count(*) FROM chunks WHERE source_version_id = '$VERSION_ID') >= 1
 AND (SELECT count(*) FROM processor_runs WHERE source_version_id = '$VERSION_ID') >= 1
 AND (SELECT count(*) FROM process_evidence WHERE operation_id = '$OPERATION_ID') >= 1
 THEN 'phase-c-worker-proof: PASS' ELSE 'phase-c-worker-proof: FAIL' END;"
