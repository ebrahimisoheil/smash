-- Phase G: durable declarative policy, decisions, approvals, conflicts, and
-- deterministic Rule fixtures.  Enforcement remains in engrave-core and is
-- repeated at each boundary; these tables make the decision reconstructable.

ALTER TABLE rules ALTER COLUMN area_id DROP NOT NULL;
ALTER TABLE rule_versions ADD COLUMN IF NOT EXISTS version_number bigint NOT NULL DEFAULT 1;
ALTER TABLE rule_versions ADD COLUMN IF NOT EXISTS scope jsonb NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE rule_versions ADD COLUMN IF NOT EXISTS evaluation_points jsonb NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE rule_versions ADD COLUMN IF NOT EXISTS priority integer NOT NULL DEFAULT 0;
ALTER TABLE rule_versions ADD COLUMN IF NOT EXISTS locked boolean NOT NULL DEFAULT false;
ALTER TABLE rule_versions ADD COLUMN IF NOT EXISTS effective_from timestamptz;
ALTER TABLE rule_versions ADD COLUMN IF NOT EXISTS effective_until timestamptz;
ALTER TABLE rules ADD COLUMN IF NOT EXISTS environment text NOT NULL DEFAULT 'default';
ALTER TABLE rules ADD COLUMN IF NOT EXISTS owner_actor_id uuid REFERENCES actors(actor_id);

CREATE TABLE IF NOT EXISTS rule_test_cases (
    rule_test_case_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    rule_id uuid NOT NULL REFERENCES rules(rule_id),
    rule_version_id uuid NOT NULL REFERENCES rule_versions(rule_version_id),
    name text NOT NULL,
    fixture jsonb NOT NULL,
    expected_effect text NOT NULL,
    expected_rationale text NOT NULL,
    expected_next_action text NOT NULL,
    expected_envelope jsonb NOT NULL,
    expected_audit jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (rule_version_id, name)
);

CREATE TABLE IF NOT EXISTS rule_decisions (
    rule_decision_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    rule_id uuid NOT NULL REFERENCES rules(rule_id),
    rule_version_id uuid NOT NULL REFERENCES rule_versions(rule_version_id),
    actor_id uuid REFERENCES actors(actor_id),
    agent_identity_id uuid REFERENCES agent_identities(agent_identity_id),
    session_id uuid,
    area_id uuid REFERENCES areas(area_id),
    purpose text NOT NULL,
    evaluation_point text NOT NULL,
    effect text NOT NULL,
    rationale text NOT NULL,
    next_action text NOT NULL,
    envelope jsonb NOT NULL,
    outcome text NOT NULL,
    request_id text NOT NULL,
    idempotency_key text,
    occurred_at timestamptz NOT NULL DEFAULT now(),
    retention_until timestamptz
);

CREATE TABLE IF NOT EXISTS rule_approvals (
    approval_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    rule_decision_id uuid NOT NULL REFERENCES rule_decisions(rule_decision_id),
    operation_hash text NOT NULL,
    approver_actor_id uuid NOT NULL REFERENCES actors(actor_id),
    state text NOT NULL,
    rule_version_id uuid NOT NULL REFERENCES rule_versions(rule_version_id),
    approved_at timestamptz,
    expires_at timestamptz,
    UNIQUE (tenant_id, rule_decision_id, operation_hash)
);

CREATE TABLE IF NOT EXISTS rule_conflicts (
    conflict_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    rule_ids jsonb NOT NULL,
    request_context jsonb NOT NULL,
    state text NOT NULL,
    review_operation_id uuid,
    resolution jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    resolved_at timestamptz
);

CREATE TABLE IF NOT EXISTS rule_review_operations (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    conflict_id uuid NOT NULL REFERENCES rule_conflicts(conflict_id),
    idempotency_key text NOT NULL,
    response jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, conflict_id, idempotency_key)
);

CREATE OR REPLACE FUNCTION require_rule_activation_admission() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state = 'active' AND current_setting('app.rule_admission', true) IS DISTINCT FROM 'approved' THEN
        RAISE EXCEPTION 'Rule activation requires approved rule admission';
    END IF;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS rule_activation_admission_guard ON rules;
CREATE TRIGGER rule_activation_admission_guard BEFORE INSERT OR UPDATE ON rules
    FOR EACH ROW EXECUTE FUNCTION require_rule_activation_admission();

CREATE INDEX IF NOT EXISTS rule_decisions_tenant_time_idx ON rule_decisions (tenant_id, occurred_at);
CREATE INDEX IF NOT EXISTS rule_versions_active_idx ON rule_versions (tenant_id, priority);

DO $$
DECLARE table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY['rule_test_cases','rule_decisions','rule_approvals','rule_conflicts','rule_review_operations'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
        EXECUTE format('CREATE POLICY %I ON %I USING (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid) WITH CHECK (tenant_id = nullif(current_setting(''app.tenant_id'', true), '''')::uuid)', table_name || '_tenant_isolation', table_name);
    END LOOP;
END;
$$;
