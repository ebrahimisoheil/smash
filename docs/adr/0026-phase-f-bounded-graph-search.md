# ADR-0026: Phase F bounded, deterministic graph search

## Status

Accepted for Phase F Session F3 on `v2-publish-f`.

## Decision

Session F3 adds `crates/core/src/graph.rs`: a pure, storage-free
`bounded_traverse` function implementing the "bounded graph expansion"
primitive already specified at a design level in
`docs/contracts/retrieval-algorithm.md` §6 (BFS depth 2, hard caps, never
unbounded). It was built concurrently with Sessions F2 and F4 as an
independent problem domain — it has no dependency on `entities.rs`,
`maps.rs`, or `cross_map.rs`, only on the shared `Entity`/`Relationship`
contract types from Session F0.

**No hidden fan-out.** `bounded_traverse(start, entities, relationships,
budget)` takes an already-authorized, single-Area slice of data and a
`start` set; it has no fetching capability and structurally cannot exceed
what it is given. Authorization and Cross-Map (cross-Area) expansion both
happen outside this function, before it is ever called — this module proves
only the traversal mechanics, not the authorization boundary around it.

**Determinism is structural, not incidental.** Every ordering-sensitive step
(adjacency construction, frontier expansion, edge selection) sorts on
`(relation_kind, neighbor EntityId, RelationshipId)` using `BTreeMap`/
`BTreeSet` rather than relying on input `Vec` order, with the UUIDv7-ordered
typed ID as the always-available final tie-break (Phase F Progress
non-negotiable: "Traversal is deterministic: typed UUIDv7 identity is the
final tie-breaker").

**`GraphBudget::default()`** is `max_depth: 2` (matching the retrieval
contract's baseline), `max_nodes: 200`, `max_edges: 400` — explicitly
documented as a measured starting point, not permanent truth, in the same
posture `docs/phase-e-ledger.md` takes toward its own tunable defaults. Any
one of the three limits being hit sets `GraphPacket::truncated = true`
rather than silently returning an incomplete-looking-complete result.

**Lifecycle state is not consulted here.** `EntityState`/`RelationshipState`
filtering (retired, superseded, rejected) is the caller's responsibility
before invoking this function, consistent with the "authorization happens
before, this module only walks what it's handed" boundary.

## Evidence

- `crates/core/src/graph.rs` — `GraphBudget`, `GraphNode`, `GraphEdge`,
  `GraphPacket`, `bounded_traverse`, and 5 unit tests: depth-limited 2-hop
  traversal, node/edge cap enforcement with `truncated` set, determinism
  under reversed input ordering, disconnected-entity exclusion, and
  empty-`start` handling.
- `crates/core/src/lib.rs` — re-exports the module.
- `cargo test --workspace --locked` — all `graph::tests::*` pass alongside
  the full existing suite.

## Not decided

No Cross-Map (cross-Area) traversal, no authorization/permission filtering,
no graph packet API route, and no benchmark evidence justifying the exact
`max_nodes`/`max_edges` defaults chosen here — those remain open (Session F4
for Cross-Map crossing rules, Session F5 for retrieval integration, Session
F6/future for a measured benchmark of the defaults).
