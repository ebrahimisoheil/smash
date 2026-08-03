-- Phase F: live PostgreSQL adapter for Maps, Entities, Relationships, and
-- Cross-Map mappings. Mirrors the Phase D memory_review_operations pattern
-- exactly: an idempotency-replay table per governed resource, plus a
-- fail-closed trigger that makes a direct SQL/API write into an activated
-- state impossible unless the application has explicitly entered the
-- corresponding admission transaction.

ALTER TABLE map_versions
    ADD COLUMN IF NOT EXISTS version bigint NOT NULL DEFAULT 1;

-- Pre-existing Phase A schema gap: `entities` never gained a `kind` column,
-- even though the Session F0 domain contract (`Entity.kind: String`) and
-- Session F2 governance (kind validated against the governing Map's
-- vocabulary) both require one. `relationships.relation_kind` already
-- existed; this is the missing counterpart for `entities`.
ALTER TABLE entities
    ADD COLUMN IF NOT EXISTS kind text NOT NULL DEFAULT '';

CREATE TABLE IF NOT EXISTS map_review_operations (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    map_version_id uuid NOT NULL REFERENCES map_versions(map_version_id),
    idempotency_key text NOT NULL,
    response jsonb NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, map_version_id, idempotency_key)
);

CREATE TABLE IF NOT EXISTS entity_review_operations (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    entity_id uuid NOT NULL REFERENCES entities(entity_id),
    idempotency_key text NOT NULL,
    response jsonb NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, entity_id, idempotency_key)
);

CREATE TABLE IF NOT EXISTS relationship_review_operations (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    relationship_id uuid NOT NULL REFERENCES relationships(relationship_id),
    idempotency_key text NOT NULL,
    response jsonb NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, relationship_id, idempotency_key)
);

CREATE TABLE IF NOT EXISTS cross_map_review_operations (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    cross_map_mapping_id uuid NOT NULL REFERENCES cross_map_mappings(cross_map_mapping_id),
    idempotency_key text NOT NULL,
    response jsonb NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, cross_map_mapping_id, idempotency_key)
);

-- Fail-closed admission guards: a direct write into an activated state
-- outside the application's governed transaction is rejected, mirroring
-- require_memory_admission() from migrations/20260804100000. This is the
-- database-side enforcement of the Phase F hard boundary against silent Map
-- publication, silent ontology activation, and implicit Cross-Map
-- activation.

CREATE OR REPLACE FUNCTION require_map_publication_admission() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state = 'published'
       AND current_setting('app.map_publication_admission', true) IS DISTINCT FROM 'approved' THEN
        RAISE EXCEPTION 'Map publication requires approved publication admission';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS map_publication_admission_guard ON map_versions;
CREATE TRIGGER map_publication_admission_guard
    BEFORE INSERT OR UPDATE ON map_versions
    FOR EACH ROW EXECUTE FUNCTION require_map_publication_admission();

CREATE OR REPLACE FUNCTION require_entity_admission() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state = 'active'
       AND current_setting('app.entity_admission', true) IS DISTINCT FROM 'approved' THEN
        RAISE EXCEPTION 'Entity activation requires approved review admission';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS entity_admission_guard ON entities;
CREATE TRIGGER entity_admission_guard
    BEFORE INSERT OR UPDATE ON entities
    FOR EACH ROW EXECUTE FUNCTION require_entity_admission();

CREATE OR REPLACE FUNCTION require_relationship_admission() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state = 'active'
       AND current_setting('app.entity_admission', true) IS DISTINCT FROM 'approved' THEN
        RAISE EXCEPTION 'Relationship activation requires approved review admission';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS relationship_admission_guard ON relationships;
CREATE TRIGGER relationship_admission_guard
    BEFORE INSERT OR UPDATE ON relationships
    FOR EACH ROW EXECUTE FUNCTION require_relationship_admission();

CREATE OR REPLACE FUNCTION require_cross_map_admission() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state = 'approved'
       AND current_setting('app.cross_map_admission', true) IS DISTINCT FROM 'approved' THEN
        RAISE EXCEPTION 'Cross-Map mapping activation requires approved review admission';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS cross_map_admission_guard ON cross_map_mappings;
CREATE TRIGGER cross_map_admission_guard
    BEFORE INSERT OR UPDATE ON cross_map_mappings
    FOR EACH ROW EXECUTE FUNCTION require_cross_map_admission();

DO $$
DECLARE table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'map_review_operations', 'entity_review_operations',
        'relationship_review_operations', 'cross_map_review_operations'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
        EXECUTE format(
            'CREATE POLICY %I ON %I USING (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid) WITH CHECK (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid)',
            table_name || '_tenant_isolation', table_name
        );
    END LOOP;
EXCEPTION WHEN duplicate_object THEN NULL;
END;
$$;
