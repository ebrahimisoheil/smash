-- Phase E performance pass: two indexes justified by measured EXPLAIN
-- ANALYZE plans against a seeded, realistically-sized local dataset (5
-- tenants x 4 areas x 20 actors x 200 memories/area = 4,000 memories, 400
-- area_grants). See docs/phase-e-ledger.md's 2026-08-09 section for the
-- exact before/after plans and dataset.

-- `memories.current_version_id` had no index at all. `search_lexical`'s
-- eligible-CTE join (`memory_versions mv JOIN memories m ON m.tenant_id =
-- mv.tenant_id AND m.current_version_id = mv.memory_version_id`) executed
-- as a Nested Loop with a Seq Scan on `memories` repeated once per matched
-- `memory_versions` row — 800 outer rows x ~800-row `memories` table =
-- 639,200 rows removed by the join filter, 86,410 buffer hits, 172.359 ms
-- execution time on only 4,000 memories. This is the authorization-
-- adjacent join path every lexical search takes; it degrades roughly
-- quadratically with tenant size, not linearly.
CREATE INDEX IF NOT EXISTS memories_current_version_idx
    ON memories (current_version_id);

-- Composite covering the same query's `memories`-side filter
-- (`tenant_id = $1 AND area_id = ANY($2) AND state = 'active'`), so the
-- planner has the option to drive the join from either side.
CREATE INDEX IF NOT EXISTS memories_tenant_area_state_idx
    ON memories (tenant_id, area_id, state);

-- `area_grants` had no index beyond its primary key. The non-admin path of
-- `resolve_search_authorization` — run on every authorization resolution
-- for every non-admin search — filters
-- `tenant_id = $1 AND actor_id = $2 AND state = 'active' AND
-- effective_from <= now() AND (effective_until IS NULL OR effective_until
-- >= now())` and measured as a Seq Scan (cheap at 400 seeded rows, but this
-- table scales with tenant x actor x area grant volume, unlike the mostly-
-- fixed-size tables that already have indexes).
CREATE INDEX IF NOT EXISTS area_grants_actor_lookup_idx
    ON area_grants (tenant_id, actor_id, state, effective_from, effective_until);
