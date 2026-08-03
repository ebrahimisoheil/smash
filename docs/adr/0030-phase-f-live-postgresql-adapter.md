# ADR-0030: Phase F live PostgreSQL adapter

## Status

Accepted for Phase F Session F7 (live-adapter gap closure) on `v2-publish-f`.

## Decision

Session F7 closes the gap explicitly flagged throughout Phase F: no
PostgreSQL adapter backed Maps, Entities, Relationships, or Cross-Map
mappings. It adds `migrations/20260808100000_phase_f_live_adapter.sql` and
eight new `PgRepository` methods in `crates/storage/src/lib.rs`, mirroring
Phase D's `create_memory_proposal`/`approve_memory_proposal` pattern exactly
— runtime `sqlx::query`, explicit `tenant_id` predicates (no reliance on
RLS, matching the existing codebase-wide pattern where the app role owns
the tables and never sets `app.tenant_id`), compare-and-swap version checks,
and an idempotency-replay table per governed resource.

**Two pre-existing schema gaps were discovered and fixed, not introduced:**
`map_versions` had no `version` column for optimistic concurrency (unlike
`entities`/`relationships`/`cross_map_mappings`, which already had one from
Phase A) — added via `ALTER TABLE ... ADD COLUMN version bigint DEFAULT 1`.
`entities` had no `kind` column at all, even though the Session F0 domain
contract and Session F2 governance both require one (`relationships.
relation_kind` already existed; `entities.kind` was simply missing since
Phase A) — added the same way.

**Fail-closed admission triggers, mirroring `require_memory_admission()`
exactly**, were added for all four resources: `require_map_publication_
admission()`, `require_entity_admission()` (shared by `entities` and
`relationships` via the same `app.entity_admission` setting), and
`require_cross_map_admission()`. Each raises an exception if a row is
written into its activated state (`published`/`active`/`approved`) without
the corresponding `SET LOCAL app.*_admission = 'approved'` having been
issued in the same transaction — a database-side, not merely
application-promised, guard against the Phase F hard boundary on silent Map
publication, silent ontology activation, and implicit Cross-Map activation.

**A real ordering bug was caught and fixed before this ADR was written**: an
initial draft combined the CAS version-bump and the gated state flip into a
single `UPDATE ... SET version = version + 1, state = 'published' WHERE
...` statement, executed *before* `SET LOCAL app.*_admission = 'approved'`.
Since the admission trigger fires on any write where `NEW.state` equals the
gated value, that single statement immediately tripped the trigger and
aborted the transaction — the corrected order is: (1) CAS-check by bumping
only `version` while `state` stays unchanged (the trigger does not fire,
since `NEW.state` isn't the gated value), (2) `SET LOCAL` the admission
flag, (3) a second, narrower `UPDATE ... SET state = '<activated>'` that now
passes the trigger. This was caught by actually running the new tests
against a live PostgreSQL instance (via `docker compose`), not merely by
`cargo check` compiling successfully — a compile-clean but logically broken
first draft would have shipped without that step.

**A second, unrelated environment defect was found and fixed in passing**:
the local `.env` file (gitignored, not committed, a personal copy of
`.env.example`) had corrupted all-caps Postgres credentials
(`POSTGRES_USER=ENGRAVE_app` instead of `engrave_app`), left over from
before this session. Regenerating it from `.env.example` and recreating the
disposable `postgres` volume fixed it; this was pre-existing local-machine
state, not a code or migration defect.

## Evidence

- `migrations/20260808100000_phase_f_live_adapter.sql` — schema gap fixes,
  four idempotency-operation tables (RLS-enabled, tenant-isolated), four
  fail-closed admission triggers.
- `compose.yaml` — the `migrate` service now mounts and runs this migration
  as step 007, after the six existing migrations.
- `crates/storage/src/lib.rs` — `create_map_draft`/`publish_map_draft`,
  `create_entity`/`approve_entity`, `create_relationship`/
  `approve_relationship`, `create_cross_map_mapping`/
  `approve_cross_map_mapping`.
- `crates/storage/tests/live_phase_f.rs` — three new `#[ignore]`d live
  tests, run for real against a disposable `docker compose` PostgreSQL
  instance (not merely written and left unexecuted): `map_draft_publish_
  replay_conflict_and_tenant_scope_are_live` (including a direct-SQL bypass
  attempt proving the admission trigger, not just the CAS check, blocks
  silent publication), `entity_and_relationship_create_and_approve_are_
  live_and_tenant_scoped`, `cross_map_mapping_create_and_approve_are_live_
  and_preserve_paths` (asserts every path/relation field is unchanged
  across propose → approve). All three pass, along with the pre-existing
  `live_repository.rs` (2 tests) and `live_queue.rs` (2 tests) live suites,
  against a **freshly created, zero-to-current-schema migrated** database —
  proving the full migration chain (001 through 007) applies cleanly from
  nothing, not just incrementally onto already-migrated state.
- `cargo test --workspace --locked` (non-live suite), `cargo clippy
  --workspace --all-targets --locked -- -D warnings`, `cargo deny check`,
  `./scripts/check-openapi.sh`, `docker compose config --quiet`, and
  `apps/web`'s `npm run build` all pass.

## Not decided

The API layer (`crates/api/src/main.rs`) is **not** wired to these new
`PgRepository` methods — this matches the pre-existing precedent that even
Phase D's own `create_proposal`/`review_proposal` HTTP handlers use only the
in-memory `MemoryStore`, never `PgRepository`'s live
`create_memory_proposal`/`approve_memory_proposal` (whose only caller in
the whole repository, before this session, was `live_repository.rs`).
Wiring any HTTP handler to a live adapter — Memory's or Phase F's — is a
separate, larger decision (conditional `AppState` construction, `Area`-grant
resolution for real authorization, OpenAPI implications) intentionally left
for a future session. `Entity.merged_into` (same-identity lineage) has no
database column and is not persisted by `approve_entity`/`create_entity` —
the live adapter proves create/approve CAS+idempotency+tenant-scoping
parity with Memory's own live adapter, not 100% parity with every action
`EntityStore`'s in-memory reference implementation supports (Memory's own
live adapter has the same characteristic: it has no live path for
`Reject`/`Edit`/`Archive`/`Restore`/`Expire`/`Supersede`, only `create`+
`approve`).
