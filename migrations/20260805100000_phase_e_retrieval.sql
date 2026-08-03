-- Phase E: rebuildable retrieval projections and resumable embedding work.
-- PostgreSQL remains canonical for authorization, lifecycle, and claim data.

CREATE TABLE IF NOT EXISTS memory_vector_projections (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    memory_id uuid NOT NULL REFERENCES memories(memory_id),
    provider text NOT NULL,
    model text NOT NULL,
    model_version text NOT NULL,
    dimension integer NOT NULL CHECK (dimension > 0),
    projection_version text NOT NULL,
    configuration_fingerprint text NOT NULL,
    content_hash text NOT NULL,
    vector jsonb NOT NULL,
    state text NOT NULL CHECK (state IN ('current', 'stale', 'failed', 'deleted')),
    updated_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, memory_id, projection_version)
);

CREATE INDEX IF NOT EXISTS memory_vector_projection_scope_idx
    ON memory_vector_projections (tenant_id, area_id, provider, model, model_version, dimension, state);

CREATE TABLE IF NOT EXISTS chunk_vector_projections (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    area_id uuid NOT NULL REFERENCES areas(area_id),
    chunk_id uuid NOT NULL REFERENCES chunks(chunk_id),
    provider text NOT NULL,
    model text NOT NULL,
    model_version text NOT NULL,
    dimension integer NOT NULL CHECK (dimension > 0),
    projection_version text NOT NULL,
    configuration_fingerprint text NOT NULL,
    content_hash text NOT NULL,
    vector jsonb NOT NULL,
    state text NOT NULL CHECK (state IN ('current', 'stale', 'failed', 'deleted')),
    updated_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, chunk_id, projection_version)
);

CREATE TABLE IF NOT EXISTS retrieval_index_jobs (
    operation_id uuid PRIMARY KEY REFERENCES operations(operation_id),
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    projection_version text NOT NULL,
    provider text NOT NULL,
    model text NOT NULL,
    model_version text NOT NULL,
    dimension integer NOT NULL CHECK (dimension > 0),
    state text NOT NULL CHECK (state IN ('queued', 'running', 'succeeded', 'failed')),
    next_cursor text,
    processed_count integer NOT NULL DEFAULT 0,
    total_count integer NOT NULL DEFAULT 0,
    last_error text,
    updated_at timestamptz NOT NULL
);

ALTER TABLE memory_vector_projections ENABLE ROW LEVEL SECURITY;
ALTER TABLE chunk_vector_projections ENABLE ROW LEVEL SECURITY;
ALTER TABLE retrieval_index_jobs ENABLE ROW LEVEL SECURITY;

DO $$
BEGIN
    EXECUTE 'CREATE POLICY memory_vector_projection_tenant_isolation ON memory_vector_projections USING (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid) WITH CHECK (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid)';
    EXECUTE 'CREATE POLICY chunk_vector_projection_tenant_isolation ON chunk_vector_projections USING (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid) WITH CHECK (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid)';
    EXECUTE 'CREATE POLICY retrieval_index_job_tenant_isolation ON retrieval_index_jobs USING (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid) WITH CHECK (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid)';
EXCEPTION WHEN duplicate_object THEN NULL;
END;
$$;
