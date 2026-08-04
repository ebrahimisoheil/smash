-- Phase I: durable, tenant-scoped aggressive-search traces. The JSON trace is
-- a reproducible snapshot; operations remain the worker lifecycle authority.
CREATE TABLE IF NOT EXISTS search_traces (
    trace_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    operation_id uuid NOT NULL REFERENCES operations(operation_id),
    actor_id uuid NOT NULL REFERENCES actors(actor_id),
    host_id text NOT NULL,
    agent_identity_id uuid NOT NULL REFERENCES agent_identities(agent_identity_id),
    session_id uuid NOT NULL,
    area_id uuid NOT NULL REFERENCES areas(area_id),
    purpose text NOT NULL,
    task text NOT NULL,
    state text NOT NULL,
    descriptor jsonb NOT NULL,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE (tenant_id, operation_id)
);
CREATE TABLE IF NOT EXISTS search_trace_steps (
    trace_id uuid NOT NULL REFERENCES search_traces(trace_id),
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    ordinal integer NOT NULL,
    kind text NOT NULL,
    area_id uuid NOT NULL REFERENCES areas(area_id),
    descriptor jsonb NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (trace_id, ordinal)
);
CREATE INDEX IF NOT EXISTS search_traces_identity_idx ON search_traces (tenant_id, actor_id, agent_identity_id, session_id, area_id);
ALTER TABLE search_traces ENABLE ROW LEVEL SECURITY;
ALTER TABLE search_trace_steps ENABLE ROW LEVEL SECURITY;
DO $$ BEGIN
    CREATE POLICY search_traces_tenant_isolation ON search_traces USING (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid) WITH CHECK (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid);
    CREATE POLICY search_trace_steps_tenant_isolation ON search_trace_steps USING (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid) WITH CHECK (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid);
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
