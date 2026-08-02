-- Phase C: durable worker lifecycle and source-pipeline evidence.
-- All writes remain tenant-scoped and original Source bytes remain immutable.

ALTER TABLE operations
    ADD COLUMN IF NOT EXISTS max_attempts integer NOT NULL DEFAULT 3,
    ADD COLUMN IF NOT EXISTS progress smallint NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS checkpoint jsonb,
    ADD COLUMN IF NOT EXISTS cancel_requested boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS error_code text,
    ADD COLUMN IF NOT EXISTS error_message text,
    ADD COLUMN IF NOT EXISTS idempotency_scope text,
    ADD COLUMN IF NOT EXISTS idempotency_key text;

ALTER TABLE artifacts
    ADD COLUMN IF NOT EXISTS configuration_fingerprint text NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS output_object_key text,
    ADD COLUMN IF NOT EXISTS warnings jsonb NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS execution_event_id uuid REFERENCES events(event_id);

ALTER TABLE chunks
    ADD COLUMN IF NOT EXISTS artifact_id uuid REFERENCES artifacts(artifact_id),
    ADD COLUMN IF NOT EXISTS start_offset bigint,
    ADD COLUMN IF NOT EXISTS end_offset bigint;

ALTER TABLE source_versions
    ADD COLUMN IF NOT EXISTS quarantine_reason text;

CREATE TABLE IF NOT EXISTS operation_checkpoints (
    operation_id uuid NOT NULL REFERENCES operations(operation_id),
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    checkpoint_key text NOT NULL,
    payload jsonb NOT NULL,
    progress smallint NOT NULL CHECK (progress BETWEEN 0 AND 100),
    created_at timestamptz NOT NULL,
    PRIMARY KEY (operation_id, checkpoint_key)
);

CREATE TABLE IF NOT EXISTS processor_runs (
    processor_run_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    operation_id uuid NOT NULL REFERENCES operations(operation_id),
    source_version_id uuid NOT NULL REFERENCES source_versions(source_version_id),
    processor_name text NOT NULL,
    processor_version text NOT NULL,
    configuration_fingerprint text NOT NULL,
    input_hash text NOT NULL,
    state text NOT NULL,
    warnings jsonb NOT NULL DEFAULT '[]'::jsonb,
    started_at timestamptz NOT NULL,
    finished_at timestamptz
);

CREATE TABLE IF NOT EXISTS process_evidence (
    process_evidence_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    operation_id uuid NOT NULL REFERENCES operations(operation_id),
    kind text NOT NULL,
    payload jsonb NOT NULL,
    created_at timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS operations_reclaim_idx
    ON operations (tenant_id, state, lease_expires_at, created_at);
CREATE INDEX IF NOT EXISTS processor_runs_lineage_idx
    ON processor_runs (tenant_id, source_version_id, processor_name, processor_version);
CREATE INDEX IF NOT EXISTS process_evidence_operation_idx
    ON process_evidence (tenant_id, operation_id, created_at);

-- A completed command can be retried safely only when the same request hash is
-- presented. The existing primary key provides the atomic claim; this index
-- makes the intended lookup explicit for adapters.
CREATE UNIQUE INDEX IF NOT EXISTS operation_idempotency_idx
    ON operations (tenant_id, idempotency_scope, idempotency_key)
    WHERE idempotency_scope IS NOT NULL AND idempotency_key IS NOT NULL;

DO $$
DECLARE table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY['operation_checkpoints', 'processor_runs', 'process_evidence'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
        EXECUTE format(
            'CREATE POLICY %I ON %I USING (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid) WITH CHECK (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid)',
            table_name || '_tenant_isolation', table_name
        );
    END LOOP;
END;
$$;
