-- Deterministic Phase B persistence seed. Safe to run repeatedly.
BEGIN;

INSERT INTO tenants (tenant_id, slug, state, created_at, updated_at)
VALUES ('018f0000-0000-7000-8000-000000000001', 'sales-fixture', 'active', '2026-01-15T09:00:00Z', '2026-01-15T09:00:00Z')
ON CONFLICT (tenant_id) DO UPDATE SET state = EXCLUDED.state, updated_at = EXCLUDED.updated_at;
INSERT INTO actors (actor_id, tenant_id, issuer, subject, state, version)
VALUES ('018f0000-0000-7000-8000-000000000030', '018f0000-0000-7000-8000-000000000001', 'fixture', 'sales-admin', 'active', 1)
ON CONFLICT (actor_id) DO NOTHING;
INSERT INTO agent_identities (agent_identity_id, tenant_id, slug, state, scopes, version)
VALUES ('018f0000-0000-7000-8000-000000000031', '018f0000-0000-7000-8000-000000000001', 'sales-agent', 'active', '{"areas":["sales"]}', 1)
ON CONFLICT (agent_identity_id) DO NOTHING;
INSERT INTO roles (role_id, tenant_id, role_key, state, version)
VALUES ('018f0000-0000-7000-8000-000000000032', '018f0000-0000-7000-8000-000000000001', 'enterprise_admin', 'active', 1)
ON CONFLICT (role_id) DO NOTHING;
INSERT INTO memberships (membership_id, tenant_id, actor_id, role_id, state, version)
VALUES ('018f0000-0000-7000-8000-000000000033', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000030', '018f0000-0000-7000-8000-000000000032', 'active', 1)
ON CONFLICT (membership_id) DO NOTHING;

INSERT INTO areas (area_id, tenant_id, slug, state, version)
VALUES ('018f0000-0000-7000-8000-000000000010', '018f0000-0000-7000-8000-000000000001', 'sales', 'active', 1),
       ('018f0000-0000-7000-8000-000000000020', '018f0000-0000-7000-8000-000000000001', 'marketing', 'active', 1)
ON CONFLICT (area_id) DO NOTHING;
INSERT INTO map_versions (map_version_id, tenant_id, area_id, version_number, state, definition)
VALUES ('018f0000-0000-7000-8000-000000000011', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 1, 'active', '{"fixture":"sales"}'),
       ('018f0000-0000-7000-8000-000000000021', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000020', 1, 'active', '{"fixture":"marketing"}')
ON CONFLICT (map_version_id) DO NOTHING;
UPDATE areas SET current_map_version_id = '018f0000-0000-7000-8000-000000000011' WHERE area_id = '018f0000-0000-7000-8000-000000000010';
UPDATE areas SET current_map_version_id = '018f0000-0000-7000-8000-000000000021' WHERE area_id = '018f0000-0000-7000-8000-000000000020';

INSERT INTO sources (source_id, tenant_id, area_id, state, title, version, created_at, updated_at)
VALUES ('018f0000-0000-7000-8000-000000000200', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 'ready', 'Acme discovery call', 1, '2026-01-15T09:00:00Z', '2026-01-15T09:00:00Z'),
       ('018f0000-0000-7000-8000-000000000210', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 'ready', 'Acme quarterly review', 1, '2026-01-15T09:00:00Z', '2026-01-15T09:00:00Z')
ON CONFLICT (source_id) DO NOTHING;
INSERT INTO source_versions (source_version_id, tenant_id, source_id, version_number, state, object_key, media_type, byte_size, checksum, created_at)
VALUES ('018f0000-0000-7000-8000-000000000201', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000200', 1, 'current', 'tenants/018f0000-0000-7000-8000-000000000001/sources/018f0000-0000-7000-8000-000000000200/versions/018f0000-0000-7000-8000-000000000201', 'text/vtt', 191, 'sha256:67128c742c79ef7e3c956527f84a15140fb164fb01edf51391b8e739a92dcfae', '2026-01-15T09:00:00Z'),
       ('018f0000-0000-7000-8000-000000000211', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000210', 1, 'superseded', 'tenants/018f0000-0000-7000-8000-000000000001/sources/018f0000-0000-7000-8000-000000000210/versions/018f0000-0000-7000-8000-000000000211', 'application/pdf', 218, 'sha256:a6946076beaeb23e666cc3e268cac0b34c2c281d1497aaf7952609d31043199f', '2026-01-15T09:00:00Z'),
       ('018f0000-0000-7000-8000-000000000212', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000210', 2, 'current', 'tenants/018f0000-0000-7000-8000-000000000001/sources/018f0000-0000-7000-8000-000000000210/versions/018f0000-0000-7000-8000-000000000212', 'application/pdf', 218, 'sha256:a6946076beaeb23e666cc3e268cac0b34c2c281d1497aaf7952609d31043199f', '2026-01-15T09:00:00Z')
ON CONFLICT (source_version_id) DO NOTHING;

INSERT INTO chunks (chunk_id, tenant_id, source_version_id, representation, coordinate, content_hash, state)
VALUES ('018f0000-0000-7000-8000-000000000220', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000201', 'transcript', '00:03:12-00:04:01', 'blake3:chunk-call-1', 'active'),
       ('018f0000-0000-7000-8000-000000000221', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000201', 'transcript', '00:17:44-00:18:20', 'blake3:chunk-call-2', 'active')
ON CONFLICT (chunk_id) DO NOTHING;
INSERT INTO memories (memory_id, tenant_id, area_id, state, origin, version)
VALUES ('018f0000-0000-7000-8000-000000000300', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 'active', 'approved', 1),
       ('018f0000-0000-7000-8000-000000000301', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 'active', 'inferred', 1),
       ('018f0000-0000-7000-8000-000000000302', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 'active', 'approved', 1)
ON CONFLICT (memory_id) DO NOTHING;
INSERT INTO memory_versions (memory_version_id, tenant_id, memory_id, version_number, state, claim, valid_from, valid_until, supersession_reason)
VALUES ('018f0000-0000-7000-8000-000000000310', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000300', 1, 'active', 'Acme requires quarterly executive review before renewal.', '2026-01-01T00:00:00Z', '2026-12-31T23:59:59Z', NULL),
       ('018f0000-0000-7000-8000-000000000311', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000301', 1, 'active', 'Acme does not require executive review before renewal.', '2026-01-01T00:00:00Z', '2026-12-31T23:59:59Z', NULL),
       ('018f0000-0000-7000-8000-000000000312', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000302', 1, 'active', 'Acme requires quarterly executive review and a security sign-off before renewal.', '2026-01-01T00:00:00Z', '2026-12-31T23:59:59Z', 'Security sign-off was added after the January review.')
ON CONFLICT (memory_version_id) DO NOTHING;
UPDATE memories SET current_version_id = '018f0000-0000-7000-8000-000000000310' WHERE memory_id = '018f0000-0000-7000-8000-000000000300';
UPDATE memories SET current_version_id = '018f0000-0000-7000-8000-000000000311' WHERE memory_id = '018f0000-0000-7000-8000-000000000301';
UPDATE memories SET current_version_id = '018f0000-0000-7000-8000-000000000312' WHERE memory_id = '018f0000-0000-7000-8000-000000000302';

INSERT INTO evidence_links (evidence_link_id, tenant_id, memory_version_id, source_version_id, chunk_id, coordinate, state)
VALUES ('018f0000-0000-7000-8000-000000000320', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000310', '018f0000-0000-7000-8000-000000000201', '018f0000-0000-7000-8000-000000000220', '00:03:12-00:04:01', 'active'),
       ('018f0000-0000-7000-8000-000000000321', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000310', '018f0000-0000-7000-8000-000000000212', NULL, NULL, 'active')
ON CONFLICT (evidence_link_id) DO NOTHING;
INSERT INTO proposals (proposal_id, tenant_id, area_id, state, origin, kind, payload, version)
VALUES ('018f0000-0000-7000-8000-000000000400', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 'pending', 'proposed', 'memory', '{"fixture":"pending"}', 1),
       ('018f0000-0000-7000-8000-000000000401', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 'approved', 'approved', 'relationship', '{"fixture":"approved"}', 1),
       ('018f0000-0000-7000-8000-000000000402', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 'rejected', 'proposed', 'memory', '{"fixture":"rejected"}', 1)
ON CONFLICT (proposal_id) DO NOTHING;

INSERT INTO rules (rule_id, tenant_id, area_id, state, version)
VALUES ('018f0000-0000-7000-8000-000000000500', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000010', 'active', 1)
ON CONFLICT (rule_id) DO NOTHING;
INSERT INTO rule_versions (rule_version_id, tenant_id, rule_id, version_number, effect, condition, rationale)
VALUES ('018f0000-0000-7000-8000-000000000501', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000500', 1, 'block', '{"case":"private evidence reuse"}', 'Private evidence cannot be reused externally'),
       ('018f0000-0000-7000-8000-000000000502', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000500', 2, 'require_approval', '{"case":"external renewal action"}', 'External renewal requires approval')
ON CONFLICT (rule_version_id) DO NOTHING;
UPDATE rules SET current_version_id = '018f0000-0000-7000-8000-000000000502' WHERE rule_id = '018f0000-0000-7000-8000-000000000500';

INSERT INTO events (event_id, tenant_id, actor_id, agent_identity_id, action, target_type, target_id, request_id, idempotency_key, occurred_at)
VALUES ('018f0000-0000-7000-8000-000000000600', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000030', '018f0000-0000-7000-8000-000000000031', 'create', 'source', '018f0000-0000-7000-8000-000000000200', 'sales-fixture-1', 'sales-source-call-create', '2026-01-15T09:01:00Z'),
       ('018f0000-0000-7000-8000-000000000601', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000030', '018f0000-0000-7000-8000-000000000031', 'approve', 'memory', '018f0000-0000-7000-8000-000000000300', 'sales-fixture-2', 'sales-memory-approve', '2026-01-15T09:02:00Z'),
       ('018f0000-0000-7000-8000-000000000602', '018f0000-0000-7000-8000-000000000001', '018f0000-0000-7000-8000-000000000030', '018f0000-0000-7000-8000-000000000031', 'supersede', 'memory', '018f0000-0000-7000-8000-000000000302', 'sales-fixture-3', 'sales-memory-supersede', '2026-01-15T09:03:00Z')
ON CONFLICT (event_id) DO NOTHING;

INSERT INTO idempotency_keys (tenant_id, scope, key, request_hash, response, created_at)
VALUES ('018f0000-0000-7000-8000-000000000001', 'fixture', 'sales-memory-approve', 'fixture-hash', '{"result":"018f0000-0000-7000-8000-000000000300"}', '2026-01-15T09:02:00Z')
ON CONFLICT (tenant_id, scope, key) DO NOTHING;
COMMIT;
