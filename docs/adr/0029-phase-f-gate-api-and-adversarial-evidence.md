# ADR-0029: Phase F gate — Board/graph/Map/Cross-Map API surface and adversarial evidence

## Status

Accepted for Phase F Session F6 (phase gate) on `v2-publish-f`.

## Decision

Session F6 adds the Board/graph/Map/Cross-Map review API contracts required
to close Phase F, plus the adversarial test suite proving the phase's
authorization, blocking, versioning, traversal, and leakage invariants hold
end-to-end through the HTTP layer, not just inside `engrave-core`.

**Route shape mirrors the existing Memory proposal routes exactly**, not a
new pattern: `crates/api/src/main.rs` gains `POST /v1/maps/drafts`,
`POST /v1/maps/drafts/{id}/review`, `POST /v1/entities`,
`POST /v1/entities/{id}/review`, `GET /v1/entities/identity-groups`,
`POST /v1/relationships`, `POST /v1/relationships/{id}/review`,
`POST /v1/graph/query`, `POST /v1/cross-map/mappings`, and
`POST /v1/cross-map/mappings/{id}/review`. Each is a thin Axum handler over
an in-memory `Arc<Mutex<...Store>>` in `AppState`, gated by the same
`x-actor-id` header check (`actor()`) the Memory routes already use. **This
is honestly a reference surface, not the fully authorization-enforced live
path** `/v1/search` has via `PgRepository::resolve_search_authorization` —
recorded explicitly as a known gap below, not glossed over, per the roadmap
requirement that "Board, graph, and Map review surfaces remain honest about
approval state."

**Deliberately not added to `ApiDoc`'s `paths(...)` list**, matching the
pre-existing precedent that `/v1/memory/proposals` and its `/review`
endpoint are *also* not in the published OpenAPI paths today — this is not
a new gap introduced by Phase F, it is consistent with how this repository
already treats early-stage mutation routes. `./scripts/check-openapi.sh`
therefore shows no diff.

**Two small, additive core-module changes support the API layer:**
`cross_map.rs`'s `CrossMapProposalInput` and `CrossMapReviewAction` gained
`Deserialize`/`Serialize` derives (they had none — Session F4 was built and
tested entirely as core-internal Rust values), and `entities.rs` gained
`EntityStore::entities_in_area`/`relationships_in_area` read accessors so
the graph-query route has something to hand `bounded_traverse`. Neither
changes any existing behavior; both are additive and covered by the full
existing test suites still passing unmodified.

**Graph query boundary**: `POST /v1/graph/query` reads only
`entities_in_area(area_id)`/`relationships_in_area(area_id)` for the
*requested* Area before calling `bounded_traverse` — a `start` id from a
different Area is structurally invisible to the traversal (proven by
`graph_query_is_bounded_area_scoped_and_never_leaks_across_areas`), matching
`graph.rs`'s own "no hidden fan-out" guarantee at the HTTP boundary too.

## Evidence

- `crates/api/src/main.rs` — 10 new routes, `AppState` gains `maps`,
  `entities`, `cross_map` stores, and 5 new adversarial `#[tokio::test]`s:
  - `map_draft_requires_actor_and_publication_is_immutable` — unauthenticated
    rejection (401) and post-publication `Edit` rejection (422).
  - `map_review_rejects_stale_version_with_conflict` — stale
    `expected_version` yields 409.
  - `entity_and_relationship_kind_validation_is_enforced` — unknown
    kind/relation both yield 422 through the HTTP layer.
  - `cross_map_requires_independent_reviewer_and_blocked_is_terminal` —
    self-approval 403, independent-reviewer approval 200, `Block` 200, and a
    post-block `Approve` attempt 422 (terminal).
  - `graph_query_is_bounded_area_scoped_and_never_leaks_across_areas` — a
    real traversal reaching a related entity, a foreign-Area start id
    returning zero nodes (leakage), and a tiny `max_nodes` budget truncating
    (bounded).
- `cargo test --workspace --locked` — 94 tests passed, 4 correctly ignored
  (require a live Postgres), 0 failed, across all seven test binaries.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
  --locked -- -D warnings`, `cargo deny check`, `./scripts/check-openapi.sh`
  (no diff), and `apps/web`'s `npm run build` all pass.

## Not decided / explicitly out of scope

No PostgreSQL adapter backs any of the ten new routes — they hold state only
for the lifetime of the API process, exactly like the existing Memory
proposal routes. No Board/Graph/Map-review **UI** was built (`apps/web`
remains the pre-existing static `/review` demo page); this ADR documents API
contracts only, consistent with how Phase D's own Memory review UI is also
still a static mock. No enforcement exists yet preventing a caller from
invoking `POST /v1/cross-map/mappings/{id}/review` with `Approve` directly
from a conversational-confirmation-driven client — that boundary is a
future UI/agent-session-contract concern (`11 Agent Session Contract.md`),
not something a stateless HTTP API can distinguish on its own. `cargo audit`
could not be run in this environment (a pre-existing non-empty
`~/.cargo/advisory-db` directory blocks the RustSec database clone); CI runs
it independently per `.github/workflows/rust-ci.yml`'s `dependency-policy`
job, which already passed on prior commits and is unaffected by this
session's changes (no new external dependencies were added — `serde`,
`serde_json`, and `time` are already-approved workspace dependencies).
