-- Phase H connector identity and cursor metadata. External payloads remain
-- worker inputs; these columns hold only stable identity and permission shape.
ALTER TABLE sources ADD COLUMN IF NOT EXISTS connector_name text;
ALTER TABLE sources ADD COLUMN IF NOT EXISTS external_id text;
ALTER TABLE sources ADD COLUMN IF NOT EXISTS connector_permissions jsonb NOT NULL DEFAULT '[]'::jsonb;
CREATE UNIQUE INDEX IF NOT EXISTS sources_connector_external_unique
    ON sources(tenant_id, connector_name, external_id)
    WHERE connector_name IS NOT NULL AND external_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS connector_sync_cursors (
    tenant_id uuid NOT NULL REFERENCES tenants(tenant_id),
    connector_name text NOT NULL,
    cursor text,
    version bigint NOT NULL DEFAULT 1,
    updated_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, connector_name)
);

-- Baseline decisions have no matching Rule. Preserve the durable audit row
-- without inventing a synthetic Rule identity.
ALTER TABLE rule_decisions ALTER COLUMN rule_id DROP NOT NULL;
ALTER TABLE rule_decisions ALTER COLUMN rule_version_id DROP NOT NULL;
ALTER TABLE rule_decisions ADD COLUMN IF NOT EXISTS host_id text;
