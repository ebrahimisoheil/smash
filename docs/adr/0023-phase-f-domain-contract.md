# ADR-0023: Phase F Map/Entity/Relationship/Cross-Map domain contract

## Status

Accepted for Phase F Session F0 on `v2-publish-f`.

## Decision

Session F0 adds the Rust type contract for versioned Area-local Maps,
Entities, Relationships, and Cross-Map mappings to `engrave-contracts`,
mirroring the DDL already shipped in Phase A
(`migrations/20260802120000_initial_schema.sql`: `map_versions`, `entities`,
`relationships`, `cross_map_mappings`). No governed mutation logic (draft to
published publication review, entity/relationship proposal flow, bounded
graph traversal, or Cross-Map approval) is implemented by this decision —
those are Sessions F1 through F6.

`MapDefinition` is a structured `{ kinds: Vec<MapKindDefinition>, relations:
Vec<MapRelationDefinition> }` value stored as the `definition` jsonb column.
Kinds and relations are opaque string keys the Map itself owns; Session F0
does not validate `Entity.kind` or `Relationship.relation_kind` against the
governing Map version's declared kinds/relations — that check belongs to the
governed mutation path built in Session F2.

`Entity` and `Relationship` both carry `map_version_id`, matching their
database columns exactly, so every structured object retains the Map version
that interpreted it (Phase F Progress non-negotiable decision #2). Once a
`MapVersion.state` is `Published`, its `definition` is immutable; a change
requires a new `version_number`, never an update to the same row (Phase F
Progress non-negotiable decision #1).

`CrossMapMapping` pins both `source_map_version_id` and
`target_map_version_id`, so a later Map version change cannot silently widen
the meaning of an already-approved mapping. Its lifecycle adds a new
`CrossMapMappingState` enum with variants `proposed`, `approved`, `rejected`,
`blocked`, `expired`, `revoked`, `superseded` — covering exactly the states
the Phase F Progress ledger requires excluded from active retrieval (Phase F
Progress non-negotiable decision #7). This is separate from the existing
`CrossMapRelation` enum (`equivalent_to`, `same_identity`, `broader_than`,
`narrower_than`, `related_to`, `derived_from`, `blocked`), which names the
semantic relation a mapping asserts, not its approval lifecycle.

## Evidence

- `crates/contracts/src/lib.rs` — `MapKindDefinition`, `MapRelationDefinition`,
  `MapDefinition`, `MapVersion`, `Entity`, `Relationship`,
  `CrossMapMappingState`, `CrossMapMapping`, and three unit tests proving
  serialization stability and Map-version retention.
- `eval/fixtures/sales/fixture.toml` — two published `map_versions`, two
  `entities`, one `relationships` row, and a `cross_map` mapping pinned to
  both Map versions.
- `crates/core/tests/sales_fixture.rs` —
  `every_entity_and_relationship_retains_a_published_map_version` asserts
  every Entity, Relationship, and the Cross-Map mapping reference a declared,
  published Map version.
- `cargo test --workspace --locked` runs these checks in CI alongside the
  existing Phase D/E suites.

## Not decided

No Map publication workflow, entity/relationship admission policy, bounded
graph traversal bound, or Cross-Map approval/expiry/revocation mechanism is
decided by this ADR. Those are Session F1 (versioned Maps), F2 (entities and
relationships), F3 (bounded graph search), and F4 (Cross-Map mappings).
