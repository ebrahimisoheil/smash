-- Phase B canonical schema. Business behavior remains in core; this migration
-- owns durable identity, tenancy, lineage, version, event, and idempotency
-- invariants. It is intentionally forward-only.
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE tenants (
    tenant_id uuid PRIMARY KEY,
    slug text NOT NULL UNIQUE,
    state text NOT NULL,
    version bigint NOT NULL DEFAULT 1,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL
);

CREATE TABLE actors (
    actor_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    issuer text NOT NULL,
    subject text NOT NULL,
    state text NOT NULL,
    version bigint NOT NULL DEFAULT 1,
    UNIQUE (tenant_id, issuer, subject)
);

CREATE TABLE roles (
    role_id uuid PRIMARY KEY,
    tenant_id uuid REFERENCES tenants(tenant_id),
    role_key text NOT NULL,
    state text NOT NULL,
    version bigint NOT NULL DEFAULT 1,
    UNIQUE (tenant_id, role_key)
);

CREATE TABLE memberships (
    membership_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    actor_id uuid NOT NULL REFERENCES actors(actor_id),
    role_id uuid NOT NULL REFERENCES roles(role_id),
    state text NOT NULL,
    version bigint NOT NULL DEFAULT 1,
    UNIQUE (tenant_id, actor_id, role_id)
);

CREATE TABLE agent_identities (
    agent_identity_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    slug text NOT NULL,
    state text NOT NULL,
    scopes jsonb NOT NULL DEFAULT '{}'::jsonb,
    version bigint NOT NULL DEFAULT 1,
    UNIQUE (tenant_id, slug)
);

CREATE TABLE areas (
    area_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    slug text NOT NULL,
    state text NOT NULL,
    current_map_version_id uuid,
    version bigint NOT NULL DEFAULT 1,
    UNIQUE (tenant_id, slug)
);

CREATE TABLE area_grants (
    area_grant_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    actor_id uuid REFERENCES actors(actor_id),
    agent_identity_id uuid REFERENCES agent_identities(agent_identity_id),
    session_id uuid,
    scope jsonb NOT NULL DEFAULT '{}'::jsonb,
    state text NOT NULL,
    effective_from timestamptz NOT NULL,
    effective_until timestamptz,
    version bigint NOT NULL DEFAULT 1,
    CHECK ((actor_id IS NOT NULL) <> (agent_identity_id IS NOT NULL))
);

CREATE TABLE placements (
    placement_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    state text NOT NULL,
    descriptor jsonb NOT NULL,
    version bigint NOT NULL DEFAULT 1
);

CREATE TABLE map_versions (
    map_version_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    version_number bigint NOT NULL,
    state text NOT NULL,
    definition jsonb NOT NULL,
    UNIQUE (area_id, version_number)
);

ALTER TABLE areas ADD CONSTRAINT areas_current_map_fk
    FOREIGN KEY (current_map_version_id) REFERENCES map_versions(map_version_id);

CREATE TABLE sources (
    source_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    state text NOT NULL,
    title text,
    version bigint NOT NULL DEFAULT 1,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL
);

CREATE TABLE source_versions (
    source_version_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    source_id uuid NOT NULL REFERENCES sources(source_id),
    version_number bigint NOT NULL,
    state text NOT NULL,
    object_key text NOT NULL,
    media_type text NOT NULL,
    byte_size bigint NOT NULL,
    checksum text NOT NULL,
    created_at timestamptz NOT NULL,
    UNIQUE (source_id, version_number),
    UNIQUE (source_id, checksum)
);

CREATE UNIQUE INDEX source_versions_one_current
    ON source_versions(source_id) WHERE state = 'current';

CREATE TABLE artifacts (
    artifact_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    source_version_id uuid NOT NULL REFERENCES source_versions(source_version_id),
    processor_name text NOT NULL,
    processor_version text NOT NULL,
    input_hash text NOT NULL,
    state text NOT NULL,
    descriptor jsonb NOT NULL,
    UNIQUE (source_version_id, processor_name, processor_version, input_hash)
);

CREATE TABLE chunks (
    chunk_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    source_version_id uuid NOT NULL REFERENCES source_versions(source_version_id),
    representation text NOT NULL,
    coordinate text NOT NULL,
    content_hash text NOT NULL,
    state text NOT NULL,
    UNIQUE (source_version_id, representation, coordinate, content_hash)
);

CREATE TABLE entities (
    entity_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    map_version_id uuid NOT NULL REFERENCES map_versions(map_version_id),
    state text NOT NULL,
    origin text NOT NULL,
    descriptor jsonb NOT NULL,
    version bigint NOT NULL DEFAULT 1
);

CREATE TABLE relationships (
    relationship_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    map_version_id uuid NOT NULL REFERENCES map_versions(map_version_id),
    source_entity_id uuid NOT NULL REFERENCES entities(entity_id),
    target_entity_id uuid NOT NULL REFERENCES entities(entity_id),
    relation_kind text NOT NULL,
    state text NOT NULL,
    origin text NOT NULL,
    version bigint NOT NULL DEFAULT 1
);

CREATE TABLE memories (
    memory_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    state text NOT NULL,
    origin text NOT NULL,
    current_version_id uuid,
    version bigint NOT NULL DEFAULT 1
);

CREATE TABLE memory_versions (
    memory_version_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    memory_id uuid NOT NULL REFERENCES memories(memory_id),
    version_number bigint NOT NULL,
    state text NOT NULL,
    claim text NOT NULL,
    valid_from timestamptz,
    valid_until timestamptz,
    predecessor_id uuid REFERENCES memory_versions(memory_version_id),
    supersession_reason text,
    UNIQUE (memory_id, version_number)
);

ALTER TABLE memories ADD CONSTRAINT memories_current_version_fk
    FOREIGN KEY (current_version_id) REFERENCES memory_versions(memory_version_id);

CREATE TABLE evidence_links (
    evidence_link_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    memory_version_id uuid NOT NULL REFERENCES memory_versions(memory_version_id),
    source_version_id uuid REFERENCES source_versions(source_version_id),
    chunk_id uuid REFERENCES chunks(chunk_id),
    coordinate text,
    state text NOT NULL,
    UNIQUE (memory_version_id, source_version_id, chunk_id, coordinate),
    CHECK ((source_version_id IS NOT NULL) OR (chunk_id IS NOT NULL))
);

CREATE TABLE proposals (
    proposal_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    state text NOT NULL,
    origin text NOT NULL,
    kind text NOT NULL,
    payload jsonb NOT NULL,
    rejection_reason text,
    version bigint NOT NULL DEFAULT 1
);

CREATE TABLE rules (
    rule_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    state text NOT NULL,
    current_version_id uuid,
    version bigint NOT NULL DEFAULT 1
);

CREATE TABLE rule_versions (
    rule_version_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    rule_id uuid NOT NULL REFERENCES rules(rule_id),
    version_number bigint NOT NULL,
    effect text NOT NULL,
    condition jsonb NOT NULL,
    rationale text NOT NULL,
    UNIQUE (rule_id, version_number)
);

ALTER TABLE rules ADD CONSTRAINT rules_current_version_fk
    FOREIGN KEY (current_version_id) REFERENCES rule_versions(rule_version_id);

CREATE TABLE events (
    event_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    actor_id uuid REFERENCES actors(actor_id),
    agent_identity_id uuid REFERENCES agent_identities(agent_identity_id),
    action text NOT NULL,
    target_type text NOT NULL,
    target_id text NOT NULL,
    previous_version bigint,
    resulting_version bigint,
    reason text,
    policy_result jsonb,
    request_id text NOT NULL,
    idempotency_key text,
    occurred_at timestamptz NOT NULL,
    schema_version smallint NOT NULL DEFAULT 1
);

CREATE TABLE operations (
    operation_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    state text NOT NULL,
    attempt integer NOT NULL DEFAULT 0,
    lease_token text,
    lease_expires_at timestamptz,
    payload jsonb NOT NULL,
    version bigint NOT NULL DEFAULT 1,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL
);

CREATE TABLE idempotency_keys (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    scope text NOT NULL,
    key text NOT NULL,
    request_hash text NOT NULL,
    response jsonb,
    created_at timestamptz NOT NULL,
    expires_at timestamptz,
    PRIMARY KEY (tenant_id, scope, key)
);

CREATE TABLE ai_runs (
    ai_run_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    actor_id uuid REFERENCES actors(actor_id),
    agent_identity_id uuid REFERENCES agent_identities(agent_identity_id),
    state text NOT NULL,
    task text NOT NULL,
    descriptor jsonb NOT NULL,
    version bigint NOT NULL DEFAULT 1,
    started_at timestamptz NOT NULL,
    finished_at timestamptz
);

CREATE TABLE decision_envelopes (
    decision_envelope_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    ai_run_id uuid NOT NULL REFERENCES ai_runs(ai_run_id),
    state text NOT NULL,
    snapshot_object_key text,
    descriptor jsonb NOT NULL,
    version bigint NOT NULL DEFAULT 1,
    UNIQUE (ai_run_id)
);

CREATE TABLE cross_map_mappings (
    cross_map_mapping_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    source_area_id uuid NOT NULL REFERENCES areas(area_id),
    target_area_id uuid NOT NULL REFERENCES areas(area_id),
    source_map_version_id uuid NOT NULL REFERENCES map_versions(map_version_id),
    target_map_version_id uuid NOT NULL REFERENCES map_versions(map_version_id),
    relation text NOT NULL,
    state text NOT NULL,
    rationale text NOT NULL,
    version bigint NOT NULL DEFAULT 1
);

CREATE INDEX tenant_actor_idx ON actors(tenant_id);
CREATE INDEX area_tenant_idx ON areas(tenant_id);
CREATE INDEX source_tenant_area_idx ON sources(tenant_id, area_id);
CREATE INDEX source_version_tenant_idx ON source_versions(tenant_id, source_id);
CREATE INDEX chunk_source_version_idx ON chunks(tenant_id, source_version_id);
CREATE INDEX event_tenant_time_idx ON events(tenant_id, occurred_at);
CREATE INDEX operation_queue_idx ON operations(tenant_id, state, created_at);

CREATE OR REPLACE FUNCTION reject_event_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'events are append-only';
END;
$$;

CREATE TRIGGER events_append_only BEFORE UPDATE OR DELETE ON events
    FOR EACH ROW EXECUTE FUNCTION reject_event_mutation();

DO $$
DECLARE table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'actors','roles','memberships','agent_identities','areas','area_grants',
        'placements','map_versions','sources','source_versions','artifacts','chunks',
        'entities','relationships','memories','memory_versions','evidence_links',
        'proposals','rules','rule_versions','events','operations','idempotency_keys',
        'ai_runs','decision_envelopes','cross_map_mappings'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
        EXECUTE format(
            'CREATE POLICY %I ON %I USING (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid) WITH CHECK (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid)',
            table_name || '_tenant_isolation', table_name
        );
    END LOOP;
END;
$$;
