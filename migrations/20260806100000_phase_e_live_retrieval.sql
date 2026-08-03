-- Phase E live retrieval adapters.
-- PostgreSQL remains authoritative. The generated document is only a bounded
-- lexical projection and never contains raw Source bodies.

ALTER TABLE memory_versions
    ADD COLUMN IF NOT EXISTS owner_actor_id uuid REFERENCES actors(actor_id),
    ADD COLUMN IF NOT EXISTS reason text NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS evidence jsonb NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS applies_when text NOT NULL DEFAULT 'always',
    ADD COLUMN IF NOT EXISTS scope text NOT NULL DEFAULT 'area',
    ADD COLUMN IF NOT EXISTS claim_hash text NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS search_document tsvector;

CREATE OR REPLACE FUNCTION refresh_memory_search_document() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    NEW.search_document := to_tsvector(
        'simple', concat_ws(' ', NEW.claim, NEW.reason, NEW.applies_when)
    );
    RETURN NEW;
END;
$$;

UPDATE memory_versions
SET search_document = to_tsvector(
    'simple', concat_ws(' ', claim, reason, applies_when)
)
WHERE search_document IS NULL;

DROP TRIGGER IF EXISTS memory_search_document_refresh ON memory_versions;
CREATE TRIGGER memory_search_document_refresh
    BEFORE INSERT OR UPDATE OF claim, reason, applies_when
    ON memory_versions
    FOR EACH ROW EXECUTE FUNCTION refresh_memory_search_document();

CREATE INDEX IF NOT EXISTS memory_versions_search_document_idx
    ON memory_versions USING GIN (search_document);

CREATE INDEX IF NOT EXISTS memory_versions_live_retrieval_idx
    ON memory_versions (tenant_id, memory_id, state, scope, owner_actor_id, valid_from, valid_until);
