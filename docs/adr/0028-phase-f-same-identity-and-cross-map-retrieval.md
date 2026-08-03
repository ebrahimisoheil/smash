# ADR-0028: Phase F reversible same-identity grouping and Cross-Map Light Search integration

## Status

Accepted for Phase F Session F5 on `v2-publish-f`.

## Decision

Session F5 has two independent halves, both extending existing Phase F/E
modules rather than adding new ones — this is the first Phase F session
built sequentially rather than fanned out, since both halves depend on
Sessions F2–F4's output.

### Reversible same-identity grouping (`crates/core/src/entities.rs`)

Adds `EntityReviewAction::Unmerge` (reverses a prior `Merge`, restoring
`EntityState::Active` and clearing the merge link) and
`EntityStore::identity_groups()` (a pure, read-only projection that resolves
each entity's `merged_into` chain to its current canonical entity and groups
members accordingly, returning only groups with more than one member).
Neither operation deletes or rewrites an Area-local record — `identity_groups`
is presentation-only, and every member stays independently readable via
`entity()` before, during, and after grouping or unmerging. This is the
direct implementation of Phase F Progress non-negotiable decision #6
("Same-identity grouping never deletes or rewrites Area-local canonical
records") and the roadmap acceptance criterion "Same-identity merges can be
reversed without losing Area-local records."

### Cross-Map Light Search expansion (`crates/core/src/retrieval.rs`)

Adds `expand_cross_map_candidates`, a new top-level function, deliberately
**not** a change to `light_search` or `SearchRequest` — non-negotiable
decision #9 ("Phase F does not reopen Phase D admission semantics or
silently alter Phase E ranking semantics") is enforced by construction:
existing `light_search` callers and all of its existing tests are
byte-for-byte unaffected, since nothing routes into the new function
automatically. It is opt-in: a caller must explicitly gather already-
approved `CrossMapExpansionSource` values and invoke the function separately.

The function re-derives traversability itself via `cross_map::is_traversable`
rather than trusting a caller's earlier check, matching non-negotiable
decision #5 ("Mapping permission is checked before candidate generation,
never only after traversal") — even though the caller is expected to have
already filtered, mapping permission is checked again immediately before
candidates are produced. It also independently rejects a mapping whose
*semantic* relation is `CrossMapRelation::Blocked`, as defense in depth
alongside the *lifecycle*-state check.

**Direction is one-way and explicit**: a mapping only expands when the
active Area equals the mapping's declared `source_area_id`, never the
reverse — Cross-Map crossing is conservative by construction, not
bidirectional by accident. Every returned `RetrievalResult` keeps the
record's original `area_id` (where it actually lives, in the *target* Area)
rather than relabeling it to the active Area, and its `warnings` field
carries an explicit `cross_map:{mapping_id}:{relation}:{source}->{target}`
path string — satisfying "Cross-Map results preserve original labels and
mapping paths."

`CrossMapExpansionBudget` defaults to `max_mappings: 3,
max_candidates_per_mapping: 5, max_total_candidates: 10` — deliberately
small, documented as a starting point rather than permanent truth, so
Cross-Map expansion cannot become an unbounded aggressive-search path hiding
inside Light Search.

## Evidence

- `crates/core/src/entities.rs` — `EntityReviewAction::Unmerge`,
  `EntityStore::identity_groups`, `IdentityGroup`, and 2 new tests: a merge
  followed by a verified `identity_groups()` result followed by `Unmerge`
  restoring `Active` and clearing the group; `Unmerge` on a non-merged entity
  rejected with `InvalidState`. 14 `entities::tests::*` total, all passing.
- `crates/core/src/retrieval.rs` — `expand_cross_map_candidates`,
  `CrossMapExpansionBudget`, `CrossMapExpansionSource`, and 4 new tests:
  original-Area/mapping-path preservation, the exhaustive non-`Approved`
  state matrix (all six other states yield zero candidates), expired /
  wrong-direction / semantically-blocked-relation rejection, and budget
  truncation. 17 `retrieval::tests::*` total (13 prior unchanged + 4 new),
  all passing.
- `cargo test --workspace --locked` — 69 `engrave-core` unit tests total (63
  prior + 6 new), full workspace suite green, `cargo clippy`/`cargo deny`/
  `./scripts/check-openapi.sh`/`apps/web` build all pass with no diff.

## Not decided

No PostgreSQL adapter or API route for either half. `bounded_traverse`
(Session F3) is not yet wired to call `expand_cross_map_candidates` for
multi-hop cross-Area graph walks — F5 only proves single-hop Cross-Map
expansion at the retrieval-candidate level, which is what "Light Search
Cross-Map expansion is opt-in and bounded" requires; deeper cross-Area graph
traversal beyond one hop remains out of scope for Light Search by design
(that is Aggressive Search's territory, explicitly out of Phase F's bounds
per the Phase F Agent Handoff Prompt). No UI/API surface exists yet to let a
caller actually assemble `CrossMapExpansionSource` values from live data —
that is Session F6's Board/graph/Map review contract work.
