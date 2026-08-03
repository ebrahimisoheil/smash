-- Phase D: governed Memory admission and review evidence.
-- A proposal is the only durable write boundary.  The trigger below makes a
-- direct SQL/API write to an active Memory fail closed unless the application
-- has explicitly entered the admission transaction.

ALTER TABLE memory_versions
    ADD COLUMN IF NOT EXISTS scope text NOT NULL DEFAULT 'area',
    ADD COLUMN IF NOT EXISTS applies_when text NOT NULL DEFAULT 'always',
    ADD COLUMN IF NOT EXISTS reason text NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS evidence jsonb NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS claim_hash text NOT NULL DEFAULT '';

ALTER TABLE proposals
    ADD COLUMN IF NOT EXISTS review_metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS reviewed_by uuid,
    ADD COLUMN IF NOT EXISTS reviewed_at timestamptz;

CREATE TABLE IF NOT EXISTS memory_review_operations (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    proposal_id uuid NOT NULL REFERENCES proposals(proposal_id),
    idempotency_key text NOT NULL,
    request_hash text NOT NULL,
    response jsonb NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, proposal_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS memory_versions_admission_idx
    ON memory_versions (tenant_id, scope, applies_when, claim_hash);

CREATE OR REPLACE FUNCTION require_memory_admission() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state = 'current'
       AND current_setting('app.memory_admission', true) IS DISTINCT FROM 'approved' THEN
        RAISE EXCEPTION 'durable Memory requires approved proposal admission';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS memory_admission_guard ON memory_versions;
CREATE TRIGGER memory_admission_guard
    BEFORE INSERT OR UPDATE ON memory_versions
    FOR EACH ROW EXECUTE FUNCTION require_memory_admission();

DO $$
BEGIN
    EXECUTE 'ALTER TABLE memory_review_operations ENABLE ROW LEVEL SECURITY';
    EXECUTE format(
        'CREATE POLICY memory_review_operations_tenant_isolation ON memory_review_operations USING (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid) WITH CHECK (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid)'
    );
EXCEPTION WHEN duplicate_object THEN NULL;
END;
$$;
