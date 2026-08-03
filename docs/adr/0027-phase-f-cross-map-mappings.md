# ADR-0027: Phase F Cross-Map mapping lifecycle

## Status

Accepted for Phase F Session F4 on `v2-publish-f`.

## Decision

Session F4 adds `crates/core/src/cross_map.rs`: a storage-free
`CrossMapStore` governing `CrossMapMapping` proposal, approval, expiry,
revocation, and blocking, mirroring the `maps.rs` shape. It was built
concurrently with Sessions F2 and F3 as an independent problem domain — its
only dependency is the shared `CrossMapMapping`/`CrossMapMappingState`/
`CrossMapRelation` contract types from Session F0.

**No self-approval path exists at all**, unlike Memory/Map's Personal-Area
carve-out: Cross-Map is inherently cross-authority (it spans two Areas), so
every `Approve` requires a reviewer distinct from the proposer, unconditionally
(Phase F Progress non-negotiable decision #4: "Cross-Map activation requires
explicit UI approval; conversational confirmation is insufficient" — this
module enforces the "distinct reviewer" half of that at the core level; the
"not from conversational confirmation" half is a caller/UI-boundary concern
this storage-free module cannot itself observe, and is noted as an open gap
below).

**`is_traversable` is the single, pure, read-time-side-effect-free source of
truth for whether a mapping may generate a Cross-Map candidate** — exposed
both as a free function `is_traversable(mapping, now, expiry)` and as a
`CrossMapStore` method. It returns `true` if and only if `state == Approved`
and (no expiry, or `now` has not reached it); every other state (`Proposed`,
`Rejected`, `Blocked`, `Expired`, `Revoked`, `Superseded`) always returns
`false`. This is the direct implementation of non-negotiable decision #7
("Rejected, blocked, expired, revoked, or superseded mappings cannot
participate in active retrieval") and is exhaustively tested against all
seven states.

**Expiry is bookkeeping outside the shared contract type.** `CrossMapMapping`
has no expiry field, so `CrossMapStore` keeps `expires_at` privately
alongside each mapping, set only at `Approve` time. A mapping past its expiry
is deliberately *not* lazily rewritten to `Expired` on read — `state` may
lag reality until an explicit governed action (e.g. `Revoke`) catches it up,
but `is_traversable` is always correct regardless of whether anyone has
"touched" the mapping since expiry. This keeps the predicate pure and
storage-free at the cost of the stored `state` occasionally lagging; a future
live adapter may add a scheduled `Expire` action to keep the persisted state
honest for UI display, but the safety-critical property (no traversal past
expiry) does not depend on that happening.

**`Blocked` is permanently terminal** — no further `Approve`/`Reject`/
`Revoke`/`Block` may move a blocked mapping anywhere else, matching
non-negotiable decision #7's "blocked mappings terminate traversal and never
become candidates."

**Mapping paths are preserved by construction**: `source_area_id`,
`target_area_id`, `source_map_version_id`, `target_map_version_id`, and
`relation` are never touched by any lifecycle transition — only `state`,
`version`, and the private `expires_at` bookkeeping change.

## Evidence

- `crates/core/src/cross_map.rs` — `CrossMapStore`, `CrossMapProposalInput`,
  `CrossMapReviewAction`, `CrossMapReviewError`, `CrossMapActivity`,
  `is_traversable`, and 17 unit tests: the exhaustive 7-state
  `is_traversable` matrix (including the critical past-expiry-but-still-
  `Approved` case), self-approval rejection / independent-reviewer success,
  `Blocked` terminality against every other action, idempotent replay,
  stale-version conflict, and path/relation-field preservation across
  propose → approve → revoke.
- `crates/core/src/lib.rs` — re-exports the module.
- `cargo test --workspace --locked` — all `cross_map::tests::*` pass
  alongside the full existing suite.

## Not decided

No PostgreSQL adapter, no API route, no scheduled/explicit `Expire` action to
keep persisted `state` honest for display, and no enforcement mechanism
preventing a UI-adjacent caller from wiring conversational confirmation
directly to `review(..., Approve)` — that boundary must be enforced by the
future API/UI layer, not by this storage-free core module. Cross-Map
traversal integration into bounded graph search (Session F3's
`bounded_traverse`) and into Light Search is Session F5.
