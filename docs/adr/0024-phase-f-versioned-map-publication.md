# ADR-0024: Phase F versioned Map draft/publication lifecycle

## Status

Accepted for Phase F Session F1 on `v2-publish-f`.

## Decision

Session F1 adds `crates/core/src/maps.rs`, a storage-free `MapStore` that
governs a Map version's lifecycle from Draft to Published, structurally
mirroring the existing proposal/review pattern in `crates/core/src/memory.rs`
(`MemoryStore` → `MapStore`, `AdmissionPolicy` → `MapPublicationPolicy`).

A `propose_draft` call never publishes on capture, matching the
proposal-first rule already established for Memory. Publication happens only
through `review(..., MapReviewAction::Approve)`, which is idempotent by
`(draft_id, idempotency_key)` replay, exactly like `MemoryStore::review`.

**Immutability enforcement:** once a draft's `state` becomes `Published`,
`review` rejects any further `Edit` or `Reject` with `InvalidState` — the
only remaining review-checked precondition is the idempotency replay path
for a repeated `Approve`. This is the direct implementation of Phase F
Progress non-negotiable decision #1 ("Map versions are immutable once
published").

**Reused states, no new ones:** a rejected draft transitions to
`MapState::Retired` rather than introducing a fourth `Rejected` variant.
`MapState` remains exactly `{Draft, Published, Retired}` as defined in
`crates/contracts/src/lib.rs` (ADR-0023 requires an ADR before adding a new
state; this session needs none because "rejected and never published" is a
retired draft, not a new lifecycle branch).

**Independent review:** `MapPublicationPolicy::SharedArea` requires a
reviewer distinct from the proposer, matching `AdmissionPolicy`'s existing
rule for shared Memory. `PersonalArea` may self-approve, matching the
personal-ontology-workflow decision recorded in `04 Domain Model.md` and
`11 Agent Session Contract.md`.

**Supersession, not mutation:** changing a published Map's definition
requires proposing a new draft with `predecessor: Some(published_id)`. The
new draft receives the next sequential `version_number` for that Area; the
predecessor's published row and `definition` are never touched. This
resolves part of open question #1 from the Phase F Progress ledger ("Which
Map fields are mandatory for publication") for the minimal case: a
`MapDefinition` must have at least one kind (`propose_draft` and `Edit`
both reject an empty `kinds` list with `MapReviewError::EmptyDefinition`);
`relations` may remain empty.

## Evidence

- `crates/core/src/maps.rs` — `MapPublicationPolicy`, `MapDraftInput`,
  `MapDraft`, `MapActivity`, `MapReviewAction`, `MapReviewError`, `MapStore`,
  and seven unit tests: proposal-first capture, empty-definition rejection,
  personal-vs-shared independent review, publication immutability (edit and
  reject both rejected post-publish), idempotent replay, version-numbered
  supersession without predecessor mutation, and stale-version rejection.
- `crates/core/src/lib.rs` — re-exports the new module the same way `memory`
  is re-exported.
- `cargo test --workspace --locked` — 29 `engrave-core` unit tests pass
  (22 prior + 7 new), full workspace suite green.

## Not decided

No PostgreSQL adapter, `map_versions` table wiring, `Area.current_map_version_id`
update path, or API route is added by this decision — those remain a live-
adapter gate, the same shape Phase D and Phase E each left open after their
own core-only sessions. Entity/Relationship kind/relation validation against
a `MapVersion`'s declared `MapDefinition` is deferred to Session F2.
