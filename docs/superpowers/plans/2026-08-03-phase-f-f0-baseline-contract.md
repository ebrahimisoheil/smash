# Phase F — Session F0: Baseline and Domain Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the Rust type contract for versioned Maps, Area-local Entities/Relationships, and Cross-Map mappings — matching the DDL already shipped in `migrations/20260802120000_initial_schema.sql` — plus fixture/test evidence proving every structured object retains its Map version, on a new `v2-publish-f` branch, without touching Phase D/E behavior.

**Architecture:** Pure data-contract addition to the framework-free `engrave-contracts` crate (mirrors the existing `opaque_id!`/state-enum/summary-struct pattern already used for Memory, Source, etc.). No governed mutation logic (draft→publish lifecycle, entity/relationship proposal flow) is implemented in this session — that is Sessions F1/F2. This session only proves the *shape* of the contract and that fixtures/tests can express it.

**Tech Stack:** Rust workspace (`engrave-contracts`, `engrave-core` test crate), TOML fixtures, `serde`/`utoipa`.

## Global Constraints

- Base branch: merged `master` containing Phase E (commit `ba0a5eaa`). Required branch: `v2-publish-f`. Do not push or merge without explicit instruction.
- Do not touch `/V2/` (legacy, gitignored, untracked).
- Do not modify `crates/core/src/memory.rs` or `crates/core/src/retrieval.rs` behavior — Phase F must not reopen Phase D admission semantics or alter Phase E ranking (`Phase F Progress.md` non-negotiable decision #9).
- Map versions are immutable once published; only `Draft`/`Published`/`Retired` states exist today (`MapState`, already defined in `crates/contracts/src/lib.rs:226-230`) — do not add new states without an ADR.
- Every `Entity`/`Relationship` struct must carry `map_version_id` — matches migration columns exactly (`migrations/20260802120000_initial_schema.sql:152-174`).
- New Cross-Map mapping lifecycle states must cover, at minimum: `proposed`, `approved`, `rejected`, `blocked`, `expired`, `revoked`, `superseded` (`Phase F Progress.md` non-negotiable decision #7).
- CI gates that must stay green: `cargo fmt --all -- --check`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo deny check`, `./scripts/check-openapi.sh` (no route changes in this session, so the checked-in `openapi.json` must not need regeneration — if it does, that's a signal a task overstepped scope).

---

### Task 1: Branch setup

**Files:** none (git only).

- [ ] **Step 1: Confirm current branch and clean tree**

Run: `git -C /Users/soheilebrahimi/Documents/smash status --short && git -C /Users/soheilebrahimi/Documents/smash branch --show-current`
Expected: clean tree, `master`.

- [ ] **Step 2: Create the Phase F branch from master**

Run: `git -C /Users/soheilebrahimi/Documents/smash checkout -b v2-publish-f`
Expected: `Switched to a new branch 'v2-publish-f'`.

---

### Task 2: Add `serde_json` as a runtime dependency of `engrave-contracts`

**Files:**
- Modify: `crates/contracts/Cargo.toml`

`serde_json` is currently only a `[dev-dependencies]` entry (used by the crate's own tests). The new `Entity.descriptor` and `MapDefinition` fields need it as a normal dependency because `Entity`/`Relationship.descriptor` mirror the `jsonb` columns in `entities`/`relationships`.

- [ ] **Step 1: Move/add the dependency**

Edit `crates/contracts/Cargo.toml` `[dependencies]` block to add:

```toml
serde_json = "1.0.145"
```

(Leave the existing `[dev-dependencies]` `serde_json = "1.0.145"` line — Cargo allows the same crate in both sections without conflict, but it's fine to delete it from `[dev-dependencies]` since the runtime dependency now covers dev/test builds too. Prefer deleting the dev-dependency duplicate line to keep the manifest DRY.)

- [ ] **Step 2: Confirm it builds**

Run: `cargo check -p engrave-contracts --locked`
Expected: success (no code uses it yet, but the manifest must resolve).

---

### Task 3: Add Map / Entity / Relationship / Cross-Map contract types

**Files:**
- Modify: `crates/contracts/src/lib.rs`

**Interfaces:**
- Consumes: existing `MapVersionId`, `EntityId`, `RelationshipId`, `CrossMapMappingId`, `TenantId`, `AreaId` (opaque IDs, `crates/contracts/src/lib.rs:56-81`), `MapState`, `EntityState`, `RelationshipState`, `Origin`, `CrossMapRelation` (existing enums, lines 226-230, 207-213, 216-222, 320-325, 329-337).
- Produces: `MapKindDefinition`, `MapRelationDefinition`, `MapDefinition`, `MapVersion`, `Entity`, `Relationship`, `CrossMapMappingState`, `CrossMapMapping` — these exact names/fields are what Sessions F1–F4 build on. Do not rename.

- [ ] **Step 1: Add the types**

Insert immediately after the existing `CrossMapRelation` enum (after line 337, before the `VersionToken` struct at line 339) in `crates/contracts/src/lib.rs`:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CrossMapMappingState {
    Proposed,
    Approved,
    Rejected,
    Blocked,
    Expired,
    Revoked,
    Superseded,
}

/// One kind of structured object a Map version recognizes (for example
/// "account" or "deal"). Kinds are opaque string keys; the Map, not the
/// platform, owns their vocabulary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct MapKindDefinition {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// One typed relation a Map version recognizes between two kinds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct MapRelationDefinition {
    pub key: String,
    pub label: String,
    pub source_kind: String,
    pub target_kind: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// The versioned semantic contract of kinds and relations for one Area.
/// Stored as the `definition` jsonb column on `map_versions`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct MapDefinition {
    pub kinds: Vec<MapKindDefinition>,
    pub relations: Vec<MapRelationDefinition>,
}

/// Mirrors the `map_versions` table. Once `state` is `Published`, `definition`
/// is immutable — a change requires a new `version_number`, never an update
/// to this row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct MapVersion {
    pub map_version_id: MapVersionId,
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub version_number: u32,
    pub state: MapState,
    pub definition: MapDefinition,
}

/// Mirrors the `entities` table. `map_version_id` records the Map version
/// under which this Entity was interpreted, per the Phase F non-negotiable
/// decision that structured objects always retain their Map version.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema)]
pub struct Entity {
    pub entity_id: EntityId,
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub map_version_id: MapVersionId,
    pub kind: String,
    pub state: EntityState,
    pub origin: Origin,
    #[schema(value_type = Object)]
    pub descriptor: serde_json::Value,
    pub version: u64,
}

/// Mirrors the `relationships` table.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct Relationship {
    pub relationship_id: RelationshipId,
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub map_version_id: MapVersionId,
    pub source_entity_id: EntityId,
    pub target_entity_id: EntityId,
    pub relation_kind: String,
    pub state: RelationshipState,
    pub origin: Origin,
    pub version: u64,
}

/// Mirrors the `cross_map_mappings` table. `source_map_version_id` and
/// `target_map_version_id` pin the mapping to the exact Map versions it was
/// approved against, so a later Map change cannot silently widen an
/// approved Cross-Map mapping's meaning.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
pub struct CrossMapMapping {
    pub cross_map_mapping_id: CrossMapMappingId,
    pub tenant_id: TenantId,
    pub source_area_id: AreaId,
    pub target_area_id: AreaId,
    pub source_map_version_id: MapVersionId,
    pub target_map_version_id: MapVersionId,
    pub relation: CrossMapRelation,
    pub state: CrossMapMappingState,
    pub rationale: String,
    pub version: u64,
}
```

Note: `Entity` cannot derive `Eq`/`Copy` because `serde_json::Value` is not `Eq`/`Copy` (it holds `f64`). Every other new struct derives `Eq` like their sibling summary structs.

`kind` on `Entity` is a plain `String` (not validated against `MapDefinition.kinds` in this session) — kind/relation validation against the governing Map version is Session F1/F2 behavior, not a contract-shape concern.

- [ ] **Step 2: Add contract tests**

Append to the `#[cfg(test)] mod tests` block at the end of `crates/contracts/src/lib.rs` (after `state_serialization_is_stable`):

```rust
    #[test]
    fn cross_map_mapping_state_serialization_is_stable() {
        assert_eq!(
            serde_json::to_string(&CrossMapMappingState::Superseded).unwrap(),
            "\"superseded\""
        );
        assert_eq!(
            serde_json::to_string(&CrossMapRelation::SameIdentity).unwrap(),
            "\"same_identity\""
        );
    }

    #[test]
    fn entity_and_relationship_retain_map_version() {
        let tenant_id = TenantId::new_v7();
        let area_id = AreaId::new_v7();
        let map_version_id = MapVersionId::new_v7();
        let entity = Entity {
            entity_id: EntityId::new_v7(),
            tenant_id,
            area_id,
            map_version_id,
            kind: "account".into(),
            state: EntityState::Active,
            origin: Origin::Observed,
            descriptor: serde_json::json!({ "name": "Acme" }),
            version: 1,
        };
        let relationship = Relationship {
            relationship_id: RelationshipId::new_v7(),
            tenant_id,
            area_id,
            map_version_id,
            source_entity_id: entity.entity_id,
            target_entity_id: EntityId::new_v7(),
            relation_kind: "owns".into(),
            state: RelationshipState::Active,
            origin: Origin::Observed,
            version: 1,
        };
        assert_eq!(entity.map_version_id, map_version_id);
        assert_eq!(relationship.map_version_id, map_version_id);
        let round_tripped: Entity =
            serde_json::from_str(&serde_json::to_string(&entity).unwrap()).unwrap();
        assert_eq!(round_tripped.map_version_id, map_version_id);
    }

    #[test]
    fn cross_map_mapping_pins_both_map_versions() {
        let mapping = CrossMapMapping {
            cross_map_mapping_id: CrossMapMappingId::new_v7(),
            tenant_id: TenantId::new_v7(),
            source_area_id: AreaId::new_v7(),
            target_area_id: AreaId::new_v7(),
            source_map_version_id: MapVersionId::new_v7(),
            target_map_version_id: MapVersionId::new_v7(),
            relation: CrossMapRelation::RelatedTo,
            state: CrossMapMappingState::Proposed,
            rationale: "shared account concept".into(),
            version: 1,
        };
        assert_ne!(mapping.source_map_version_id, mapping.target_map_version_id);
        assert_eq!(mapping.state, CrossMapMappingState::Proposed);
    }
```

- [ ] **Step 3: Run the contract crate's tests**

Run: `cargo test -p engrave-contracts --locked`
Expected: all tests pass, including the three new ones.

---

### Task 4: Extend the Sales fixture with Map/Entity/Relationship data

**Files:**
- Modify: `eval/fixtures/sales/fixture.toml`
- Modify: `eval/fixtures/sales/coverage.md`

The existing fixture already has a `[cross_map]` block with `state = "proposed"` (matches the new `CrossMapMappingState::Proposed`) but no explicit Map-version, Entity, or Relationship records. Add them so the fixture — the same one Phase E's benchmark and CI regression gate already load — carries Phase F baseline data.

- [ ] **Step 1: Add a `[[map_versions]]` array and `[[entities]]`/`[[relationships]]` arrays**

Insert into `eval/fixtures/sales/fixture.toml`, immediately after the existing `[[areas]]` blocks (after line 15, before `[actor]`):

```toml
[[map_versions]]
id = "018f0000-0000-7000-8000-000000000011"
area_id = "018f0000-0000-7000-8000-000000000010"
version_number = 1
state = "published"
kinds = ["account", "opportunity", "person"]
relations = ["owns", "employs"]

[[map_versions]]
id = "018f0000-0000-7000-8000-000000000021"
area_id = "018f0000-0000-7000-8000-000000000020"
version_number = 1
state = "published"
kinds = ["campaign", "account"]
relations = ["targets"]
```

Insert a new section after the existing `[opportunity]` block (after line 124, before `[[sources]]`):

```toml
[[entities]]
id = "018f0000-0000-7000-8000-000000000700"
area_id = "018f0000-0000-7000-8000-000000000010"
map_version_id = "018f0000-0000-7000-8000-000000000011"
kind = "account"
state = "active"
origin = "observed"

[[entities]]
id = "018f0000-0000-7000-8000-000000000701"
area_id = "018f0000-0000-7000-8000-000000000010"
map_version_id = "018f0000-0000-7000-8000-000000000011"
kind = "person"
state = "active"
origin = "observed"

[[relationships]]
id = "018f0000-0000-7000-8000-000000000710"
area_id = "018f0000-0000-7000-8000-000000000010"
map_version_id = "018f0000-0000-7000-8000-000000000011"
source_entity_id = "018f0000-0000-7000-8000-000000000701"
target_entity_id = "018f0000-0000-7000-8000-000000000700"
relation_kind = "owns"
state = "active"
origin = "observed"
```

Update the existing `[cross_map]` block (lines 149-154) to pin both Map versions, matching the new `CrossMapMapping` contract:

```toml
[cross_map]
id = "018f0000-0000-7000-8000-000000000600"
state = "proposed"
source_area_id = "018f0000-0000-7000-8000-000000000010"
target_area_id = "018f0000-0000-7000-8000-000000000020"
source_map_version_id = "018f0000-0000-7000-8000-000000000011"
target_map_version_id = "018f0000-0000-7000-8000-000000000021"
relation = "related_to"
rationale = "Sales account concept may enrich the Marketing account view."
```

- [ ] **Step 2: Update the coverage table row count**

`eval/fixtures/sales/coverage.md` is asserted by `sales_fixture.rs:247-254` to have exactly 13 data rows (rows starting with `|`, minus the 2 header rows). Read the file, add one row documenting the new Map/Entity/Relationship/Cross-Map-pinning coverage, and update the count assertion in Task 5 Step 1 to match the new total (14).

---

### Task 5: Assert the fixture proves Map-version retention

**Files:**
- Modify: `crates/core/tests/sales_fixture.rs`

**Interfaces:**
- Consumes: `engrave_contracts::{MapState}` (new import), the fixture TOML shape added in Task 4.

- [ ] **Step 1: Add Rust structs for the new TOML sections**

In `crates/core/tests/sales_fixture.rs`, add to the `Fixture` struct (after `areas: Vec<Area>,` at line 13):

```rust
    map_versions: Vec<MapVersionFixture>,
```

and after the `entities`/`relationships` position — add fields `entities: Vec<EntityFixture>` and `relationships: Vec<RelationshipFixture>` to `Fixture` right after `opportunity: Record,` (line 16):

```rust
    entities: Vec<EntityFixture>,
    relationships: Vec<RelationshipFixture>,
```

Add the struct definitions after the `Area` struct (after line 40):

```rust
#[derive(Debug, Deserialize)]
struct MapVersionFixture {
    id: String,
    area_id: String,
    version_number: u32,
    state: String,
    kinds: Vec<String>,
    relations: Vec<String>,
}
#[derive(Debug, Deserialize)]
struct EntityFixture {
    id: String,
    area_id: String,
    map_version_id: String,
    kind: String,
    state: String,
    origin: Origin,
}
#[derive(Debug, Deserialize)]
struct RelationshipFixture {
    id: String,
    area_id: String,
    map_version_id: String,
    source_entity_id: String,
    target_entity_id: String,
    relation_kind: String,
    state: String,
    origin: Origin,
}
```

Update the `CrossMap` struct (lines 108-115) to add the two new required fields:

```rust
#[derive(Debug, Deserialize)]
struct CrossMap {
    id: String,
    state: String,
    source_area_id: String,
    target_area_id: String,
    source_map_version_id: String,
    target_map_version_id: String,
    relation: String,
    rationale: String,
}
```

- [ ] **Step 2: Add the assertion test**

Append a new test function after `canonical_sales_fixture_is_deterministic_and_complete` (after line 255):

```rust
#[test]
fn every_entity_and_relationship_retains_a_published_map_version() {
    let fixture: Fixture = toml::from_str(FIXTURE).expect("fixture TOML must parse");
    assert_eq!(fixture.map_versions.len(), 2);
    assert!(fixture
        .map_versions
        .iter()
        .all(|map_version| map_version.state == "published"));
    let known_map_version_ids: std::collections::BTreeSet<_> = fixture
        .map_versions
        .iter()
        .map(|map_version| map_version.id.as_str())
        .collect();

    assert_eq!(fixture.entities.len(), 2);
    for entity in &fixture.entities {
        assert!(
            known_map_version_ids.contains(entity.map_version_id.as_str()),
            "entity {} must reference a declared map version",
            entity.id
        );
    }

    assert_eq!(fixture.relationships.len(), 1);
    for relationship in &fixture.relationships {
        assert!(
            known_map_version_ids.contains(relationship.map_version_id.as_str()),
            "relationship {} must reference a declared map version",
            relationship.id
        );
        assert!(fixture
            .entities
            .iter()
            .any(|entity| entity.id == relationship.source_entity_id));
        assert!(fixture
            .entities
            .iter()
            .any(|entity| entity.id == relationship.target_entity_id));
    }

    assert!(known_map_version_ids.contains(fixture.cross_map.source_map_version_id.as_str()));
    assert!(known_map_version_ids.contains(fixture.cross_map.target_map_version_id.as_str()));
    assert_ne!(
        fixture.cross_map.source_map_version_id,
        fixture.cross_map.target_map_version_id
    );
}
```

- [ ] **Step 3: Update the row-count assertion from Task 4 Step 2**

In `canonical_sales_fixture_is_deterministic_and_complete`, change:

```rust
    assert_eq!(
        COVERAGE
            .lines()
            .filter(|line| line.starts_with('|'))
            .count()
            - 2,
        13
    );
```

to `14` (matching the coverage row added in Task 4 Step 2).

- [ ] **Step 4: Run the test**

Run: `cargo test -p engrave-core --test sales_fixture --locked`
Expected: 3 tests pass (`canonical_sales_fixture_is_deterministic_and_complete`, `phase_e_benchmark_manifest_is_deterministic_and_authorization_first`, `every_entity_and_relationship_retains_a_published_map_version`).

---

### Task 6: Record the domain contract decision and full verification

**Files:**
- Create: `docs/adr/0023-phase-f-domain-contract.md`
- Modify: `ENGRAVE V2/Plans/Phase F Progress.md` (Obsidian vault — via `mcp__kika-obsidian__update_note`, not a repo file)

- [ ] **Step 1: Write the ADR**

Create `docs/adr/0023-phase-f-domain-contract.md` documenting: the `MapDefinition` (kinds+relations) shape chosen, the `CrossMapMappingState` variant set and why (`proposed/approved/rejected/blocked/expired/revoked/superseded` — matches Phase F Progress non-negotiable decision #7), and that `Entity.kind`/`Relationship.relation_kind` are unvalidated strings in this session (validation against the governing `MapDefinition` is deferred to Session F2). Follow the format of `docs/adr/0022-phase-e-retrieval-contract.md`.

- [ ] **Step 2: Run full verification suite**

```bash
git branch --show-current
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo deny check
./scripts/check-openapi.sh
```

and from `apps/web`:

```bash
npm ci --ignore-scripts && npm run build
```

Expected: all pass; `check-openapi.sh` passes with **no diff** (this session adds no API routes/schemas reachable from `crates/api`, so `openapi.json` should not need regeneration — if it does, stop and investigate before proceeding, since that means a type leaked into the API surface unexpectedly).

- [ ] **Step 3: Commit**

```bash
git add crates/contracts/Cargo.toml crates/contracts/src/lib.rs \
  eval/fixtures/sales/fixture.toml eval/fixtures/sales/coverage.md \
  crates/core/tests/sales_fixture.rs docs/adr/0023-phase-f-domain-contract.md
git commit -m "Add Phase F Map/Entity/Relationship/Cross-Map domain contract"
```

- [ ] **Step 4: Update the Phase F ledger**

Use `mcp__kika-obsidian__update_note` on `ENGRAVE V2/Plans/Phase F Progress.md`: mark session F0 `complete*` in the Sessions table, add a "Completed session F0" section (mirroring the Phase E Progress ledger's format) with exact files changed, delivered evidence, and exact verification command output/results, and record the commit hash from Step 3.

---

## Self-review notes

- Spec coverage: this plan covers only the F0 row of the Phase F Progress ledger ("Map/Entity/Relationship/Cross-Map contract, fixtures, branch setup" / "Existing Phase E gate remains green; new branch confirmed"). It deliberately does **not** cover F1 (draft/publish governance), F2 (entity/relationship proposal lifecycle), F3 (bounded graph traversal), F4 (Cross-Map approval/revocation), F5 (same-identity merge), or F6 (phase gate) — those are separate future sessions/plans, consistent with the ledger's own session breakdown.
- No placeholders: every step has literal code/TOML/commands.
- Type consistency checked: `Entity.map_version_id` / `Relationship.map_version_id` / `CrossMapMapping.{source,target}_map_version_id` are all `MapVersionId`; fixture test fields use matching `String` (UUID text) representations, consistent with how the existing `Area.map_version_id: String` fixture field already works.
