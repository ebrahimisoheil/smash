# ADR-0025: Phase F Area-local Entity and Relationship governance

## Status

Accepted for Phase F Session F2 on `v2-publish-f`.

## Decision

Session F2 adds `crates/core/src/entities.rs`, a storage-free `EntityStore`
governing Area-local Entity and Relationship proposals, mirroring the
proposal-first, idempotent-replay, independent-review shape already
established by `memory.rs` and `maps.rs`. The module intentionally has no
compile-time dependency on `maps.rs` or `memory.rs` — it was built
concurrently with Sessions F3 and F4 as an independent problem domain, and
keeping it dependency-free from its siblings is what made that safe.

**Kind/relation validation against the governing Map, deferred from F0/F1, is
implemented here.** `propose_entity` rejects an unknown `kind` against a
caller-supplied `governing_kinds: Vec<String>`; `propose_relationship`
rejects an unknown `relation_kind` against a caller-supplied
`governing_relations: Vec<MapRelationDefinition>`, and additionally rejects a
dangling `source_entity_id`/`target_entity_id` or a kind mismatch against the
relation definition's declared `source_kind`/`target_kind`. The caller (a
future live adapter) is responsible for resolving the correct governing
`MapDefinition` from a published `MapVersion` before calling — this module
does not reach into `maps.rs` to do that lookup itself.

**Retire and Merge are transitions, never deletions**, per non-negotiable
decision #6 ("Same-identity grouping never deletes or rewrites Area-local
canonical records"). `EntityState` has no `Rejected` variant in the
contracts crate, so `EntityReviewAction::Reject` also transitions to
`EntityState::Retired` (documented in the module) rather than requiring a new
contract state; `RelationshipState` does have `Rejected`, so relationship
rejection uses it directly. `Merge` records the target entity id
(`merged_into`) without removing the original record — it stays fully
readable via `entity()` afterward, laying the groundwork for Session F5's
reversible same-identity grouping projection.

## Evidence

- `crates/core/src/entities.rs` — `EntityStore`, `EntityProposalPolicy`,
  `EntityDraftInput`, `RelationshipDraftInput`, `EntityReviewAction`,
  `RelationshipReviewAction`, `EntityReviewError`, `EntityActivity`, and 12
  unit tests covering proposal-first capture, unknown-kind/unknown-relation/
  dangling-reference/kind-mismatch rejection, personal-vs-shared independent
  review for both entities and relationships, idempotent replay, stale-version
  conflict, and non-destructive retire/merge.
- `crates/core/src/lib.rs` — re-exports the module the same way `memory` and
  `maps` are re-exported.
- `cargo test --workspace --locked` — all `entities::tests::*` pass alongside
  the full existing suite.

## Not decided

No PostgreSQL adapter, no live resolution of a `MapVersion`'s
`MapDefinition` to populate `governing_kinds`/`governing_relations`, no API
route, and no same-identity grouping *presentation* logic (only the
non-destructive `Merge` state transition primitive) — those remain open for
later sessions (a live-adapter gate, and Session F5 respectively).
