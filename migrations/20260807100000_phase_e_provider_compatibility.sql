-- Phase E.2: provider compatibility metadata. Secrets stay in deployment
-- secret stores; only the non-secret environment variable name is recorded.

CREATE TABLE IF NOT EXISTS embedding_provider_profiles (
    profile_name text PRIMARY KEY,
    provider text NOT NULL,
    model text NOT NULL,
    model_version text NOT NULL,
    native_dimension integer NOT NULL CHECK (native_dimension > 0),
    output_dimension integer NOT NULL CHECK (output_dimension = 1024 OR profile_name LIKE 'test-%'),
    projection_version text NOT NULL,
    configuration_fingerprint text NOT NULL,
    credential_env text,
    production boolean NOT NULL DEFAULT true,
    enabled boolean NOT NULL DEFAULT false,
    updated_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE memory_vector_projections
    ADD COLUMN IF NOT EXISTS native_dimension integer,
    ADD COLUMN IF NOT EXISTS production_profile boolean NOT NULL DEFAULT true;
ALTER TABLE chunk_vector_projections
    ADD COLUMN IF NOT EXISTS native_dimension integer,
    ADD COLUMN IF NOT EXISTS production_profile boolean NOT NULL DEFAULT true;

UPDATE memory_vector_projections SET native_dimension = dimension WHERE native_dimension IS NULL;
UPDATE chunk_vector_projections SET native_dimension = dimension WHERE native_dimension IS NULL;
ALTER TABLE memory_vector_projections ALTER COLUMN native_dimension SET NOT NULL;
ALTER TABLE chunk_vector_projections ALTER COLUMN native_dimension SET NOT NULL;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'memory_vector_output_dimension_1024') THEN
        ALTER TABLE memory_vector_projections ADD CONSTRAINT memory_vector_output_dimension_1024
            CHECK (NOT production_profile OR dimension = 1024);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chunk_vector_output_dimension_1024') THEN
        ALTER TABLE chunk_vector_projections ADD CONSTRAINT chunk_vector_output_dimension_1024
            CHECK (NOT production_profile OR dimension = 1024);
    END IF;
END;
$$;

ALTER TABLE retrieval_index_jobs
    ADD COLUMN IF NOT EXISTS native_dimension integer,
    ADD COLUMN IF NOT EXISTS lease_token text,
    ADD COLUMN IF NOT EXISTS lease_expires_at timestamptz,
    ADD COLUMN IF NOT EXISTS attempt integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS max_attempts integer NOT NULL DEFAULT 5,
    ADD COLUMN IF NOT EXISTS cancel_requested boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS checkpoint jsonb NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS dead_lettered boolean NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS retrieval_index_jobs_lease_idx
    ON retrieval_index_jobs (tenant_id, state, lease_expires_at);
