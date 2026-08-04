-- Phase J: durable, tenant-scoped workspace/ontology interview state.
-- Submission creates only a Proposal; Area grants and Map publication remain
-- separate governed operations.
CREATE TABLE IF NOT EXISTS workspace_interviews (
    interview_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    actor_id uuid NOT NULL REFERENCES actors(actor_id),
    agent_identity_id uuid NOT NULL REFERENCES agent_identities(agent_identity_id),
    session_id uuid NOT NULL,
    host_id text NOT NULL,
    purpose text NOT NULL,
    state text NOT NULL CHECK (state IN ('collecting','awaiting_confirmation','confirmed','submitted','cancelled')),
    selected_area_ids jsonb NOT NULL DEFAULT '[]'::jsonb,
    requested_areas jsonb NOT NULL DEFAULT '[]'::jsonb,
    ontology_draft jsonb NOT NULL DEFAULT '{}'::jsonb,
    confirmed boolean NOT NULL DEFAULT false,
    proposal_id uuid,
    idempotency_key text NOT NULL,
    version bigint NOT NULL DEFAULT 1,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, idempotency_key)
);
CREATE INDEX IF NOT EXISTS workspace_interviews_identity_idx
    ON workspace_interviews (tenant_id, actor_id, agent_identity_id, session_id);
ALTER TABLE workspace_interviews ENABLE ROW LEVEL SECURITY;
DO $$ BEGIN
    CREATE POLICY workspace_interviews_tenant_isolation
        ON workspace_interviews
        USING (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid)
        WITH CHECK (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid);
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
