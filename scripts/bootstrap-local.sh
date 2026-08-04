#!/usr/bin/env bash
set -euo pipefail

# Seed the minimum local identities needed to exercise the Community Edition.
# This is an operator bootstrap helper, not an authentication or provisioning
# service. It must never be used as a hosted multi-tenant control plane.
ENV_FILE="${COMPOSE_ENV_FILE:-.env}"
TENANT_SLUG="${1:-local}"
ACTOR_SUBJECT="${2:-local-operator}"
AREA_SLUG="${3:-general}"
AGENT_ONE_SLUG="${4:-agent-one}"
AGENT_TWO_SLUG="${5:-agent-two}"

test -f "$ENV_FILE" || {
  echo "missing Compose environment file: $ENV_FILE (copy .env.example to .env first)" >&2
  exit 1
}
CONFIGURED_TENANT_ID="$(sed -n 's/^ENGRAVE_TENANT_ID[[:space:]]*=[[:space:]]*//p' "$ENV_FILE" | tail -n 1 | tr -d '\r')"
command -v docker >/dev/null 2>&1 || {
  echo "docker is required" >&2
  exit 1
}
for value in "$TENANT_SLUG" "$ACTOR_SUBJECT" "$AREA_SLUG" "$AGENT_ONE_SLUG" "$AGENT_TWO_SLUG"; do
  test -n "$value" || { echo "bootstrap values must not be empty" >&2; exit 1; }
done
if test -n "$CONFIGURED_TENANT_ID"; then
  printf '%s\n' "$CONFIGURED_TENANT_ID" | rg -i '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' >/dev/null || {
    echo "ENGRAVE_TENANT_ID in $ENV_FILE must be a UUID" >&2
    exit 1
  }
fi

uuid() {
  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen | tr '[:upper:]' '[:lower:]'
  elif command -v python3 >/dev/null 2>&1; then
    python3 -c 'import uuid; print(uuid.uuid4())'
  else
    echo "uuidgen or python3 is required to create bootstrap session IDs" >&2
    exit 1
  fi
}

actor_session="$(uuid)"
agent_one_session="$(uuid)"
agent_two_session="$(uuid)"

docker compose --env-file "$ENV_FILE" exec -T postgres sh -c \
  'exec psql --username="$POSTGRES_USER" --dbname="$POSTGRES_DB" "$@"' sh \
  --quiet --tuples-only --no-align --set=ON_ERROR_STOP=1 --no-psqlrc \
  --set=tenant_id="$CONFIGURED_TENANT_ID" \
  --set=tenant_slug="$TENANT_SLUG" \
  --set=actor_subject="$ACTOR_SUBJECT" \
  --set=area_slug="$AREA_SLUG" \
  --set=agent_one_slug="$AGENT_ONE_SLUG" \
  --set=agent_two_slug="$AGENT_TWO_SLUG" \
  --set=actor_session="$actor_session" \
  --set=agent_one_session="$agent_one_session" \
  --set=agent_two_session="$agent_two_session" <<'SQL'
BEGIN;

INSERT INTO tenants (tenant_id, slug, state, created_at, updated_at)
VALUES (
  CASE WHEN :'tenant_id' = '' THEN gen_random_uuid() ELSE :'tenant_id'::uuid END,
  :'tenant_slug', 'active', now(), now()
)
ON CONFLICT (slug) DO UPDATE SET state = 'active', updated_at = now();
SELECT tenant_id FROM tenants WHERE slug = :'tenant_slug' \gset tenant_

INSERT INTO actors (actor_id, tenant_id, issuer, subject, state)
VALUES (gen_random_uuid(), :'tenant_tenant_id', 'local-bootstrap', :'actor_subject', 'active')
ON CONFLICT (tenant_id, issuer, subject) DO UPDATE SET state = 'active';
SELECT actor_id FROM actors
WHERE tenant_id = :'tenant_tenant_id' AND issuer = 'local-bootstrap' AND subject = :'actor_subject' \gset actor_

INSERT INTO roles (role_id, tenant_id, role_key, state)
VALUES (gen_random_uuid(), :'tenant_tenant_id', 'area_admin', 'active')
ON CONFLICT (tenant_id, role_key) DO UPDATE SET state = 'active';
SELECT role_id FROM roles
WHERE tenant_id = :'tenant_tenant_id' AND role_key = 'area_admin' \gset admin_role_

INSERT INTO memberships (membership_id, tenant_id, actor_id, role_id, state)
VALUES (gen_random_uuid(), :'tenant_tenant_id', :'actor_actor_id', :'admin_role_role_id', 'active')
ON CONFLICT (tenant_id, actor_id, role_id) DO UPDATE SET state = 'active';

INSERT INTO areas (area_id, tenant_id, slug, state)
VALUES (gen_random_uuid(), :'tenant_tenant_id', :'area_slug', 'active')
ON CONFLICT (tenant_id, slug) DO UPDATE SET state = 'active';
SELECT area_id FROM areas
WHERE tenant_id = :'tenant_tenant_id' AND slug = :'area_slug' \gset area_

INSERT INTO agent_identities (agent_identity_id, tenant_id, slug, state)
VALUES (gen_random_uuid(), :'tenant_tenant_id', :'agent_one_slug', 'active')
ON CONFLICT (tenant_id, slug) DO UPDATE SET state = 'active';
SELECT agent_identity_id FROM agent_identities
WHERE tenant_id = :'tenant_tenant_id' AND slug = :'agent_one_slug' \gset agent_one_

INSERT INTO agent_identities (agent_identity_id, tenant_id, slug, state)
VALUES (gen_random_uuid(), :'tenant_tenant_id', :'agent_two_slug', 'active')
ON CONFLICT (tenant_id, slug) DO UPDATE SET state = 'active';
SELECT agent_identity_id FROM agent_identities
WHERE tenant_id = :'tenant_tenant_id' AND slug = :'agent_two_slug' \gset agent_two_

INSERT INTO area_grants
  (area_grant_id, tenant_id, area_id, actor_id, scope, state, effective_from)
SELECT gen_random_uuid(), :'tenant_tenant_id', :'area_area_id', :'actor_actor_id', '{}'::jsonb, 'active', now()
WHERE NOT EXISTS (
  SELECT 1 FROM area_grants
  WHERE tenant_id = :'tenant_tenant_id' AND area_id = :'area_area_id'
    AND actor_id = :'actor_actor_id' AND state = 'active'
);

INSERT INTO area_grants
  (area_grant_id, tenant_id, area_id, agent_identity_id, scope, state, effective_from)
SELECT gen_random_uuid(), :'tenant_tenant_id', :'area_area_id', :'agent_one_agent_identity_id', '{}'::jsonb, 'active', now()
WHERE NOT EXISTS (
  SELECT 1 FROM area_grants
  WHERE tenant_id = :'tenant_tenant_id' AND area_id = :'area_area_id'
    AND agent_identity_id = :'agent_one_agent_identity_id' AND state = 'active'
);

INSERT INTO area_grants
  (area_grant_id, tenant_id, area_id, agent_identity_id, scope, state, effective_from)
SELECT gen_random_uuid(), :'tenant_tenant_id', :'area_area_id', :'agent_two_agent_identity_id', '{}'::jsonb, 'active', now()
WHERE NOT EXISTS (
  SELECT 1 FROM area_grants
  WHERE tenant_id = :'tenant_tenant_id' AND area_id = :'area_area_id'
    AND agent_identity_id = :'agent_two_agent_identity_id' AND state = 'active'
);

COMMIT;

SELECT json_build_object(
  'tenant_id', :'tenant_tenant_id',
  'area_id', :'area_area_id',
  'contexts', json_build_object(
    'actor', json_build_object(
      'tenant_id', :'tenant_tenant_id',
      'actor_id', :'actor_actor_id',
      'host_id', 'local-bootstrap',
      'agent_identity_id', :'agent_one_agent_identity_id',
      'session_id', :'actor_session',
      'purpose', 'local administration',
      'role', 'area_admin',
      'area_id', :'area_area_id',
      'environment', 'local'
    ),
    'agent_one', json_build_object(
      'tenant_id', :'tenant_tenant_id',
      'actor_id', :'actor_actor_id',
      'host_id', 'local-agent-one',
      'agent_identity_id', :'agent_one_agent_identity_id',
      'session_id', :'agent_one_session',
      'purpose', 'local evidence work',
      'role', 'normal_user',
      'area_id', :'area_area_id',
      'environment', 'local'
    ),
    'agent_two', json_build_object(
      'tenant_id', :'tenant_tenant_id',
      'actor_id', :'actor_actor_id',
      'host_id', 'local-agent-two',
      'agent_identity_id', :'agent_two_agent_identity_id',
      'session_id', :'agent_two_session',
      'purpose', 'local evidence work',
      'role', 'normal_user',
      'area_id', :'area_area_id',
      'environment', 'local'
    )
  )
);
SQL

cat >&2 <<'NOTICE'
Bootstrap complete. The JSON above contains local test contexts only.
Keep this deployment on localhost; this script does not configure OAuth,
identity proof, invitations, or hosted tenant isolation.
NOTICE
