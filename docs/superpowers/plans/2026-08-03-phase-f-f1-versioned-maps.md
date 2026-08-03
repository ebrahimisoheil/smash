# Phase F — Session F1: Versioned Maps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the governed draft-to-published lifecycle for `MapVersion`s: a proposer creates a Draft, an independent reviewer (for shared Areas) approves or rejects it, and once `Published` the definition is immutable — any later change creates a new, sequentially numbered draft. Prove this with deterministic unit tests, mirroring the existing `crates/core/src/memory.rs` proposal/review pattern.

**Architecture:** New storage-free module `crates/core/src/maps.rs`, structurally identical in shape to `memory.rs` (`MemoryStore` → `MapStore`, `AdmissionPolicy` → `MapPublicationPolicy`, `ProposalInput`/`ReviewAction`/`ReviewError` → `MapDraftInput`/`MapReviewAction`/`MapReviewError`). Re-exported from `crates/core/src/lib.rs` the same way `memory` is. No live PostgreSQL adapter, migration, or API route is added in this session — this is the same "core contract first, live wiring later" split Phase D and Phase E each used (their `crates/core/src/memory.rs` / `retrieval.rs` landed before `crates/storage` adapters and API routes did).

**Tech Stack:** Rust (`engrave-core`), reusing `engrave-contracts::{MapVersion, MapDefinition, MapState, MapVersionId, AreaId, TenantId}` from Session F0.

## Global Constraints

- Base branch: `v2-publish-f` (already created in Session F0, commit `b12c2b3b`). Do not push/merge without explicit instruction.
- Map versions are immutable once published; drafts remain reviewable (Phase F Progress non-negotiable decision #1). Enforce: once a draft's state is `Published`, `review()` must return `MapReviewError::InvalidState` for `Edit`/`Reject`/re-`Approve` other than idempotent replay.
- Do not add new `MapState` variants — only `Draft`, `Published`, `Retired` exist (`crates/contracts/src/lib.rs:226-230`); this session's rejected-draft semantics reuse `Retired`, they do not add a fourth `Rejected` state (ADR-0023 says "do not add new states without an ADR" — reusing existing states needs none).
- Publishing a shared-Area Map requires an independent reviewer (mirrors `AdmissionPolicy::independent_reviewer_required` in `memory.rs:19-23`); personal-Area Maps may self-approve.
- Do not touch `crates/core/src/memory.rs` or `crates/core/src/retrieval.rs` (Phase F Progress non-negotiable decision #9).
- CI gates that must stay green: `cargo fmt --all -- --check`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo deny check`, `./scripts/check-openapi.sh` (no API routes touched this session, so no diff expected).

---

### Task 1: Add the `maps` module

**Files:**
- Create: `crates/core/src/maps.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `engrave_contracts::{AreaId, MapDefinition, MapState, MapVersion, MapVersionId, TenantId}` (all exist from Session F0).
- Produces: `MapPublicationPolicy`, `MapDraftInput`, `MapDraft`, `MapActivity`, `MapReviewAction`, `MapReviewError`, `MapStore` — re-exported from `engrave_core`. Session F2 (entity/relationship proposals) and Session F4 (Cross-Map approval) will reuse this same shape.

- [ ] **Step 1: Write `crates/core/src/maps.rs`**

```rust
//! Draft-to-published Map version governance.
//!
//! This module is deliberately storage-free, mirroring `memory.rs`: it makes
//! the publication-immutability and independent-review invariants
//! executable in contract tests before any PostgreSQL adapter exists.

use engrave_contracts::{AreaId, MapDefinition, MapState, MapVersion, MapVersionId, TenantId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MapPublicationPolicy {
    PersonalArea,
    SharedArea,
}

impl MapPublicationPolicy {
    fn independent_reviewer_required(self) -> bool {
        matches!(self, Self::SharedArea)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MapDraftInput {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub proposer: String,
    pub reason: String,
    pub definition: MapDefinition,
    pub policy: MapPublicationPolicy,
    pub predecessor: Option<MapVersionId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapDraft {
    pub id: MapVersionId,
    pub input: MapDraftInput,
    pub version_number: u32,
    pub state: MapState,
    pub review_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapActivity {
    pub action: String,
    pub target_id: String,
    pub actor: String,
    pub reason: String,
    pub previous_version_number: Option<u32>,
    pub resulting_version_number: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MapReviewAction {
    Approve,
    Reject { reason: String },
    Edit(MapDefinition),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapReviewError {
    NotFound,
    InvalidState,
    VersionConflict { current_version: u64 },
    IndependentReviewRequired,
    EmptyDefinition,
}

#[derive(Default)]
pub struct MapStore {
    drafts: BTreeMap<MapVersionId, MapDraft>,
    published: BTreeMap<MapVersionId, MapVersion>,
    next_version_number: BTreeMap<AreaId, u32>,
    operations: BTreeMap<String, String>,
    activity: Vec<MapActivity>,
}

impl MapStore {
    /// Creates a reviewable draft. This function never publishes on capture,
    /// even for Personal-Area policy — publication always goes through
    /// `review`.
    pub fn propose_draft(
        &mut self,
        id: MapVersionId,
        input: MapDraftInput,
    ) -> Result<MapDraft, MapReviewError> {
        if input.definition.kinds.is_empty() {
            return Err(MapReviewError::EmptyDefinition);
        }
        let version_number = self
            .next_version_number
            .get(&input.area_id)
            .copied()
            .unwrap_or(1);
        let draft = MapDraft {
            id,
            input: input.clone(),
            version_number,
            state: MapState::Draft,
            review_version: 1,
        };
        self.activity.push(MapActivity {
            action: "propose".into(),
            target_id: id.as_uuid().to_string(),
            actor: input.proposer,
            reason: input.reason,
            previous_version_number: None,
            resulting_version_number: Some(version_number),
        });
        self.drafts.insert(id, draft.clone());
        Ok(draft)
    }

    pub fn draft(&self, id: MapVersionId) -> Option<&MapDraft> {
        self.drafts.get(&id)
    }
    pub fn published(&self, id: MapVersionId) -> Option<&MapVersion> {
        self.published.get(&id)
    }
    pub fn activity(&self) -> &[MapActivity] {
        &self.activity
    }

    pub fn review(
        &mut self,
        draft_id: MapVersionId,
        reviewer: &str,
        expected_version: u64,
        idempotency_key: &str,
        action: MapReviewAction,
    ) -> Result<Option<MapVersionId>, MapReviewError> {
        let replay_key = format!("map-draft:{}:{}", draft_id.as_uuid(), idempotency_key);
        if let Some(result) = self.operations.get(&replay_key) {
            return Ok(result.parse().ok().map(MapVersionId::new));
        }
        let draft = self.drafts.get(&draft_id).ok_or(MapReviewError::NotFound)?;
        if draft.review_version != expected_version {
            return Err(MapReviewError::VersionConflict {
                current_version: draft.review_version,
            });
        }
        if draft.state != MapState::Draft {
            return Err(MapReviewError::InvalidState);
        }
        if draft.input.policy.independent_reviewer_required() && draft.input.proposer == reviewer
        {
            return Err(MapReviewError::IndependentReviewRequired);
        }
        let result = match action {
            MapReviewAction::Approve => Some(self.publish(draft_id, reviewer)?),
            MapReviewAction::Reject { reason } => {
                self.reject(draft_id, reviewer, reason)?;
                None
            }
            MapReviewAction::Edit(definition) => {
                self.edit(draft_id, reviewer, definition)?;
                None
            }
        };
        self.operations.insert(
            replay_key,
            result
                .map(|id: MapVersionId| id.as_uuid().to_string())
                .unwrap_or_default(),
        );
        Ok(result)
    }

    fn publish(&mut self, id: MapVersionId, reviewer: &str) -> Result<MapVersionId, MapReviewError> {
        let draft = self.drafts.get(&id).cloned().ok_or(MapReviewError::NotFound)?;
        let published = MapVersion {
            map_version_id: id,
            tenant_id: draft.input.tenant_id,
            area_id: draft.input.area_id,
            version_number: draft.version_number,
            state: MapState::Published,
            definition: draft.input.definition.clone(),
        };
        let previous_version_number = draft
            .input
            .predecessor
            .and_then(|predecessor| self.published.get(&predecessor))
            .map(|map_version| map_version.version_number);
        self.published.insert(id, published);
        self.next_version_number
            .insert(draft.input.area_id, draft.version_number + 1);
        let mut updated_draft = draft.clone();
        updated_draft.state = MapState::Published;
        updated_draft.review_version += 1;
        self.drafts.insert(id, updated_draft);
        self.activity.push(MapActivity {
            action: "publish".into(),
            target_id: id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason: draft.input.reason,
            previous_version_number,
            resulting_version_number: Some(draft.version_number),
        });
        Ok(id)
    }

    fn reject(&mut self, id: MapVersionId, reviewer: &str, reason: String) -> Result<(), MapReviewError> {
        let draft = self.drafts.get_mut(&id).ok_or(MapReviewError::NotFound)?;
        draft.state = MapState::Retired;
        draft.review_version += 1;
        self.activity.push(MapActivity {
            action: "reject".into(),
            target_id: id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason,
            previous_version_number: None,
            resulting_version_number: None,
        });
        Ok(())
    }

    fn edit(
        &mut self,
        id: MapVersionId,
        reviewer: &str,
        definition: MapDefinition,
    ) -> Result<(), MapReviewError> {
        if definition.kinds.is_empty() {
            return Err(MapReviewError::EmptyDefinition);
        }
        let draft = self.drafts.get_mut(&id).ok_or(MapReviewError::NotFound)?;
        draft.input.definition = definition;
        draft.review_version += 1;
        self.activity.push(MapActivity {
            action: "update".into(),
            target_id: id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason: "draft edited before publication".into(),
            previous_version_number: None,
            resulting_version_number: None,
        });
        Ok(())
    }
}
```

- [ ] **Step 2: Wire the module into `crates/core/src/lib.rs`**

After `pub use memory::{AdmissionPolicy, MemoryStore, ProposalInput, ReviewAction, ReviewError};` (line 26), insert:

```rust
pub mod maps;
pub use maps::{
    MapActivity, MapDraft, MapDraftInput, MapPublicationPolicy, MapReviewAction, MapReviewError,
    MapStore,
};
```

- [ ] **Step 3: Confirm it compiles**

Run: `cargo check -p engrave-core --locked`
Expected: success.

---

### Task 2: Unit tests proving the lifecycle invariants

**Files:**
- Modify: `crates/core/src/maps.rs` (add `#[cfg(test)] mod tests` at the end)

- [ ] **Step 1: Add the tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use engrave_contracts::{MapKindDefinition, MapRelationDefinition};

    fn definition() -> MapDefinition {
        MapDefinition {
            kinds: vec![MapKindDefinition {
                key: "account".into(),
                label: "Account".into(),
                description: None,
            }],
            relations: vec![MapRelationDefinition {
                key: "owns".into(),
                label: "Owns".into(),
                source_kind: "person".into(),
                target_kind: "account".into(),
                description: None,
            }],
        }
    }

    fn input(proposer: &str, policy: MapPublicationPolicy) -> MapDraftInput {
        MapDraftInput {
            tenant_id: TenantId::new_v7(),
            area_id: AreaId::new_v7(),
            proposer: proposer.into(),
            reason: "initial Sales Map".into(),
            definition: definition(),
            policy,
            predecessor: None,
        }
    }

    #[test]
    fn draft_never_publishes_on_capture() {
        let mut store = MapStore::default();
        let draft = store
            .propose_draft(MapVersionId::new_v7(), input("agent", MapPublicationPolicy::PersonalArea))
            .unwrap();
        assert_eq!(draft.state, MapState::Draft);
        assert!(store.published(draft.id).is_none());
    }

    #[test]
    fn empty_definition_is_rejected() {
        let mut store = MapStore::default();
        let mut bad = input("agent", MapPublicationPolicy::PersonalArea);
        bad.definition.kinds.clear();
        assert_eq!(
            store.propose_draft(MapVersionId::new_v7(), bad),
            Err(MapReviewError::EmptyDefinition)
        );
    }

    #[test]
    fn personal_self_approval_and_shared_independent_review_are_distinct() {
        let mut store = MapStore::default();
        let personal = store
            .propose_draft(MapVersionId::new_v7(), input("agent", MapPublicationPolicy::PersonalArea))
            .unwrap();
        assert!(store
            .review(personal.id, "agent", 1, "p1", MapReviewAction::Approve)
            .unwrap()
            .is_some());

        let shared = store
            .propose_draft(MapVersionId::new_v7(), input("agent", MapPublicationPolicy::SharedArea))
            .unwrap();
        assert_eq!(
            store.review(shared.id, "agent", 1, "s1", MapReviewAction::Approve),
            Err(MapReviewError::IndependentReviewRequired)
        );
        assert!(store
            .review(shared.id, "area-admin", 1, "s1", MapReviewAction::Approve)
            .unwrap()
            .is_some());
    }

    #[test]
    fn published_definition_is_immutable() {
        let mut store = MapStore::default();
        let draft = store
            .propose_draft(MapVersionId::new_v7(), input("agent", MapPublicationPolicy::PersonalArea))
            .unwrap();
        store
            .review(draft.id, "agent", 1, "publish", MapReviewAction::Approve)
            .unwrap();
        assert_eq!(
            store.review(
                draft.id,
                "agent",
                2,
                "edit-after-publish",
                MapReviewAction::Edit(definition())
            ),
            Err(MapReviewError::InvalidState)
        );
        assert_eq!(
            store.review(draft.id, "agent", 2, "reject-after-publish", MapReviewAction::Reject { reason: "too late".into() }),
            Err(MapReviewError::InvalidState)
        );
        let published = store.published(draft.id).unwrap();
        assert_eq!(published.definition, definition());
    }

    #[test]
    fn publish_is_idempotent_and_explainable() {
        let mut store = MapStore::default();
        let draft = store
            .propose_draft(MapVersionId::new_v7(), input("agent", MapPublicationPolicy::PersonalArea))
            .unwrap();
        let id = store
            .review(draft.id, "agent", 1, "publish", MapReviewAction::Approve)
            .unwrap()
            .unwrap();
        let replay = store
            .review(draft.id, "agent", 1, "publish", MapReviewAction::Approve)
            .unwrap()
            .unwrap();
        assert_eq!(id, replay);
        assert!(store
            .activity()
            .iter()
            .any(|event| event.action == "publish" && event.resulting_version_number == Some(1)));
    }

    #[test]
    fn a_new_draft_supersedes_without_mutating_the_published_predecessor() {
        let mut store = MapStore::default();
        let area_id = AreaId::new_v7();
        let mut first_input = input("agent", MapPublicationPolicy::PersonalArea);
        first_input.area_id = area_id;
        let first = store
            .propose_draft(MapVersionId::new_v7(), first_input)
            .unwrap();
        let first_id = store
            .review(first.id, "agent", 1, "publish-v1", MapReviewAction::Approve)
            .unwrap()
            .unwrap();

        let mut revised = definition();
        revised.kinds.push(MapKindDefinition {
            key: "opportunity".into(),
            label: "Opportunity".into(),
            description: None,
        });
        let mut second_input = input("agent", MapPublicationPolicy::PersonalArea);
        second_input.area_id = area_id;
        second_input.definition = revised.clone();
        second_input.predecessor = Some(first_id);
        let second = store
            .propose_draft(MapVersionId::new_v7(), second_input)
            .unwrap();
        assert_eq!(second.version_number, 2);
        store
            .review(second.id, "agent", 1, "publish-v2", MapReviewAction::Approve)
            .unwrap();

        let first_published = store.published(first_id).unwrap();
        assert_eq!(first_published.definition, definition());
        assert_eq!(first_published.version_number, 1);
        let second_published = store.published(second.id).unwrap();
        assert_eq!(second_published.definition, revised);
        assert_eq!(second_published.version_number, 2);
    }

    #[test]
    fn review_is_rejected_on_stale_version() {
        let mut store = MapStore::default();
        let draft = store
            .propose_draft(MapVersionId::new_v7(), input("agent", MapPublicationPolicy::PersonalArea))
            .unwrap();
        assert_eq!(
            store.review(draft.id, "agent", 99, "publish", MapReviewAction::Approve),
            Err(MapReviewError::VersionConflict { current_version: 1 })
        );
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p engrave-core --lib maps:: --locked`
Expected: 7 tests pass.

---

### Task 3: Full verification, ADR, ledger, commit

**Files:**
- Create: `docs/adr/0024-phase-f-versioned-map-publication.md`
- Modify: `ENGRAVE V2/Plans/Phase F Progress.md` (Obsidian, via `mcp__kika-obsidian__update_note`)

- [ ] **Step 1: Write ADR-0024** documenting: the draft→published lifecycle, why rejected drafts reuse `MapState::Retired` instead of a new state, the independent-reviewer rule for `SharedArea`, and that supersession always creates a new `version_number` rather than mutating a published row. Note explicitly what is *not* decided: no PostgreSQL adapter, no `Area.current_map_version_id` update wiring, no API route — those are future sessions (a live-adapter gate, same shape as Phase E's `complete*` sessions).

- [ ] **Step 2: Run full verification**

```bash
git branch --show-current
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo deny check
./scripts/check-openapi.sh
```

and from `apps/web`: `npm ci --ignore-scripts && npm run build`.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/maps.rs crates/core/src/lib.rs docs/adr/0024-phase-f-versioned-map-publication.md \
  docs/superpowers/plans/2026-08-03-phase-f-f1-versioned-maps.md
git commit -m "Add Phase F versioned Map draft/publication lifecycle"
```

- [ ] **Step 4: Update the Phase F ledger** — mark F1 `complete*` in the Sessions table, add a "Completed session F1" section with files changed, delivered evidence, exact verification output, and the commit hash.

## Self-review notes

- Spec coverage: covers the F1 ledger row exactly ("Draft/published Map versions, publication review, immutable version tests" / "Every structured record retains Map version"). Does not cover F2 (entity/relationship proposals), F3 (graph), F4 (Cross-Map approval), F5 (same-identity), F6 (phase gate) — separate future sessions.
- No placeholders: every step has literal code.
- Type consistency: `MapDraft.id` / `MapVersion.map_version_id` are both `MapVersionId`; `MapDraftInput.definition`/`MapVersion.definition` are both `MapDefinition`; matches Session F0's contract exactly.
