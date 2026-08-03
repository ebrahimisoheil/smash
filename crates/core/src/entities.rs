//! Area-local Entity and Relationship governance.
//!
//! This module is deliberately storage-free, mirroring `memory.rs` and
//! `maps.rs`: it makes the proposal-first, independent-review, and
//! lineage-preservation invariants executable in contract tests before any
//! PostgreSQL adapter exists. It intentionally has no dependency on sibling
//! Phase F modules (`memory.rs`, `maps.rs`) — the caller passes in whatever
//! governing Map vocabulary is relevant as plain data.

use engrave_contracts::{
    AreaId, Entity, EntityId, EntityState, MapRelationDefinition, MapVersionId, Origin,
    Relationship, RelationshipId, RelationshipState, TenantId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityProposalPolicy {
    PersonalArea,
    SharedArea,
}

impl EntityProposalPolicy {
    fn independent_reviewer_required(self) -> bool {
        matches!(self, Self::SharedArea)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EntityDraftInput {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub map_version_id: MapVersionId,
    pub proposer: String,
    pub reason: String,
    pub kind: String,
    pub descriptor: serde_json::Value,
    pub origin: Origin,
    pub policy: EntityProposalPolicy,
    pub governing_kinds: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RelationshipDraftInput {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub map_version_id: MapVersionId,
    pub proposer: String,
    pub reason: String,
    pub source_entity_id: EntityId,
    pub target_entity_id: EntityId,
    pub relation_kind: String,
    pub origin: Origin,
    pub policy: EntityProposalPolicy,
    pub governing_relations: Vec<MapRelationDefinition>,
}

/// A same-identity grouping of Area-local Entities, resolved from `Merge`
/// lineage. Purely a presentation projection: no member is deleted, and
/// every member's own kind/descriptor/origin remain intact and independently
/// readable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityGroup {
    pub canonical: EntityId,
    pub members: Vec<EntityId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityActivity {
    pub action: String,
    pub target_id: String,
    pub actor: String,
    pub reason: String,
    pub previous_version: Option<u64>,
    pub resulting_version: Option<u64>,
    pub merged_into: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityReviewAction {
    Approve,
    Reject {
        reason: String,
    },
    Retire,
    Merge {
        into: EntityId,
    },
    /// Reverses a prior `Merge`, restoring the record to `Active`. Same-
    /// identity merging must be reversible without losing Area-local
    /// records (Phase F non-negotiable decision #6) — this is that
    /// reversal, not a new destructive action.
    Unmerge,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipReviewAction {
    Approve,
    Reject { reason: String },
    Retire,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityReviewError {
    NotFound,
    InvalidState,
    VersionConflict { current_version: u64 },
    IndependentReviewRequired,
    UnknownKind,
    UnknownRelation,
    DanglingEntityReference,
    KindMismatch,
}

#[derive(Clone, Debug, PartialEq)]
struct EntityRecord {
    entity: Entity,
    proposer: String,
    policy: EntityProposalPolicy,
    merged_into: Option<EntityId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RelationshipRecord {
    relationship: Relationship,
    proposer: String,
    policy: EntityProposalPolicy,
}

#[derive(Default)]
pub struct EntityStore {
    entities: BTreeMap<EntityId, EntityRecord>,
    relationships: BTreeMap<RelationshipId, RelationshipRecord>,
    operations: BTreeMap<String, String>,
    activity: Vec<EntityActivity>,
}

impl EntityStore {
    /// Creates a reviewable Entity proposal. This function never activates
    /// on capture, even for Personal-Area policy — activation always goes
    /// through `review_entity`.
    pub fn propose_entity(
        &mut self,
        id: EntityId,
        input: EntityDraftInput,
    ) -> Result<Entity, EntityReviewError> {
        if !input.governing_kinds.iter().any(|k| k == &input.kind) {
            return Err(EntityReviewError::UnknownKind);
        }
        let entity = Entity {
            entity_id: id,
            tenant_id: input.tenant_id,
            area_id: input.area_id,
            map_version_id: input.map_version_id,
            kind: input.kind,
            state: EntityState::Proposed,
            origin: input.origin,
            descriptor: input.descriptor,
            version: 1,
        };
        self.activity.push(EntityActivity {
            action: "propose_entity".into(),
            target_id: id.as_uuid().to_string(),
            actor: input.proposer.clone(),
            reason: input.reason,
            previous_version: None,
            resulting_version: Some(1),
            merged_into: None,
        });
        self.entities.insert(
            id,
            EntityRecord {
                entity: entity.clone(),
                proposer: input.proposer,
                policy: input.policy,
                merged_into: None,
            },
        );
        Ok(entity)
    }

    /// Creates a reviewable Relationship proposal. Never activates on
    /// capture. Validates the relation kind against the governing Map
    /// vocabulary and validates both endpoints exist with matching kinds.
    pub fn propose_relationship(
        &mut self,
        id: RelationshipId,
        input: RelationshipDraftInput,
    ) -> Result<Relationship, EntityReviewError> {
        let relation_def = input
            .governing_relations
            .iter()
            .find(|r| r.key == input.relation_kind)
            .ok_or(EntityReviewError::UnknownRelation)?;

        let source = self
            .entities
            .get(&input.source_entity_id)
            .ok_or(EntityReviewError::DanglingEntityReference)?;
        let target = self
            .entities
            .get(&input.target_entity_id)
            .ok_or(EntityReviewError::DanglingEntityReference)?;

        if source.entity.kind != relation_def.source_kind
            || target.entity.kind != relation_def.target_kind
        {
            return Err(EntityReviewError::KindMismatch);
        }

        let relationship = Relationship {
            relationship_id: id,
            tenant_id: input.tenant_id,
            area_id: input.area_id,
            map_version_id: input.map_version_id,
            source_entity_id: input.source_entity_id,
            target_entity_id: input.target_entity_id,
            relation_kind: input.relation_kind,
            state: RelationshipState::Proposed,
            origin: input.origin,
            version: 1,
        };
        self.activity.push(EntityActivity {
            action: "propose_relationship".into(),
            target_id: id.as_uuid().to_string(),
            actor: input.proposer.clone(),
            reason: input.reason,
            previous_version: None,
            resulting_version: Some(1),
            merged_into: None,
        });
        self.relationships.insert(
            id,
            RelationshipRecord {
                relationship: relationship.clone(),
                proposer: input.proposer,
                policy: input.policy,
            },
        );
        Ok(relationship)
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id).map(|record| &record.entity)
    }

    pub fn relationship(&self, id: RelationshipId) -> Option<&Relationship> {
        self.relationships
            .get(&id)
            .map(|record| &record.relationship)
    }

    /// Returns the Entity a merged Entity was merged into, if any.
    pub fn merged_into(&self, id: EntityId) -> Option<EntityId> {
        self.entities.get(&id).and_then(|record| record.merged_into)
    }

    /// A reversible, presentation-only grouping of Area-local Entities by
    /// same-identity `Merge` lineage. This never deletes, hides, or rewrites
    /// an Area-local record — every member remains independently readable
    /// via `entity()` — it only tells a caller which canonical entity a
    /// chain of merges currently resolves to, so a UI can group them for
    /// display and a reviewer can `Unmerge` any member back to `Active`.
    /// Only groups with more than one member are returned; an entity with no
    /// merge history is not "grouped" with itself.
    pub fn identity_groups(&self) -> Vec<IdentityGroup> {
        let mut groups: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
        for id in self.entities.keys().copied() {
            groups
                .entry(self.resolve_canonical(id))
                .or_default()
                .insert(id);
        }
        groups
            .into_iter()
            .filter(|(_, members)| members.len() > 1)
            .map(|(canonical, members)| IdentityGroup {
                canonical,
                members: members.into_iter().collect(),
            })
            .collect()
    }

    /// Follows `merged_into` links to the current root of a merge chain. A
    /// cycle (which a correct caller should never produce, since `Merge`
    /// requires the target to already exist and `Unmerge` clears the link)
    /// is defensively broken rather than looped forever.
    fn resolve_canonical(&self, id: EntityId) -> EntityId {
        let mut current = id;
        let mut seen = BTreeSet::from([current]);
        while let Some(target) = self
            .entities
            .get(&current)
            .and_then(|record| record.merged_into)
        {
            if !seen.insert(target) {
                break;
            }
            current = target;
        }
        current
    }

    pub fn activity(&self) -> &[EntityActivity] {
        &self.activity
    }

    pub fn review_entity(
        &mut self,
        id: EntityId,
        reviewer: &str,
        expected_version: u64,
        idempotency_key: &str,
        action: EntityReviewAction,
    ) -> Result<Entity, EntityReviewError> {
        let replay_key = format!("entity:{}:{}", id.as_uuid(), idempotency_key);
        if self.operations.contains_key(&replay_key) {
            return self
                .entities
                .get(&id)
                .map(|record| record.entity.clone())
                .ok_or(EntityReviewError::NotFound);
        }

        let record = self.entities.get(&id).ok_or(EntityReviewError::NotFound)?;
        if record.entity.version != expected_version {
            return Err(EntityReviewError::VersionConflict {
                current_version: record.entity.version,
            });
        }
        if record.policy.independent_reviewer_required() && record.proposer == reviewer {
            return Err(EntityReviewError::IndependentReviewRequired);
        }

        let current_state = record.entity.state;
        let current_version = record.entity.version;

        let (new_state, action_name, reason, merged_into) = match action {
            EntityReviewAction::Approve => {
                if current_state != EntityState::Proposed {
                    return Err(EntityReviewError::InvalidState);
                }
                (
                    EntityState::Active,
                    "approve",
                    "reviewed and approved".to_string(),
                    None,
                )
            }
            EntityReviewAction::Reject { reason } => {
                if current_state != EntityState::Proposed {
                    return Err(EntityReviewError::InvalidState);
                }
                // EntityState has no dedicated Rejected variant: a rejected
                // proposal retires the Area-local record rather than being
                // deleted or rewritten.
                (EntityState::Retired, "reject", reason, None)
            }
            EntityReviewAction::Retire => {
                if !matches!(current_state, EntityState::Proposed | EntityState::Active) {
                    return Err(EntityReviewError::InvalidState);
                }
                (
                    EntityState::Retired,
                    "retire",
                    "reviewed lifecycle transition".to_string(),
                    None,
                )
            }
            EntityReviewAction::Merge { into } => {
                if current_state != EntityState::Active {
                    return Err(EntityReviewError::InvalidState);
                }
                if !self.entities.contains_key(&into) {
                    return Err(EntityReviewError::DanglingEntityReference);
                }
                (
                    EntityState::Merged,
                    "merge",
                    format!("merged into {}", into.as_uuid()),
                    Some(into),
                )
            }
            EntityReviewAction::Unmerge => {
                if current_state != EntityState::Merged {
                    return Err(EntityReviewError::InvalidState);
                }
                (
                    EntityState::Active,
                    "unmerge",
                    "reversed same-identity merge".to_string(),
                    None,
                )
            }
        };

        let record = self
            .entities
            .get_mut(&id)
            .ok_or(EntityReviewError::NotFound)?;
        record.entity.state = new_state;
        record.entity.version += 1;
        record.merged_into = merged_into;
        let updated = record.entity.clone();

        self.activity.push(EntityActivity {
            action: action_name.into(),
            target_id: id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason,
            previous_version: Some(current_version),
            resulting_version: Some(updated.version),
            merged_into: merged_into.map(|into| into.as_uuid().to_string()),
        });
        self.operations
            .insert(replay_key, updated.version.to_string());
        Ok(updated)
    }

    pub fn review_relationship(
        &mut self,
        id: RelationshipId,
        reviewer: &str,
        expected_version: u64,
        idempotency_key: &str,
        action: RelationshipReviewAction,
    ) -> Result<Relationship, EntityReviewError> {
        let replay_key = format!("relationship:{}:{}", id.as_uuid(), idempotency_key);
        if self.operations.contains_key(&replay_key) {
            return self
                .relationships
                .get(&id)
                .map(|record| record.relationship.clone())
                .ok_or(EntityReviewError::NotFound);
        }

        let record = self
            .relationships
            .get(&id)
            .ok_or(EntityReviewError::NotFound)?;
        if record.relationship.version != expected_version {
            return Err(EntityReviewError::VersionConflict {
                current_version: record.relationship.version,
            });
        }
        if record.policy.independent_reviewer_required() && record.proposer == reviewer {
            return Err(EntityReviewError::IndependentReviewRequired);
        }

        let current_state = record.relationship.state;
        let current_version = record.relationship.version;

        let (new_state, action_name, reason) = match action {
            RelationshipReviewAction::Approve => {
                if current_state != RelationshipState::Proposed {
                    return Err(EntityReviewError::InvalidState);
                }
                (
                    RelationshipState::Active,
                    "approve",
                    "reviewed and approved".to_string(),
                )
            }
            RelationshipReviewAction::Reject { reason } => {
                if current_state != RelationshipState::Proposed {
                    return Err(EntityReviewError::InvalidState);
                }
                (RelationshipState::Rejected, "reject", reason)
            }
            RelationshipReviewAction::Retire => {
                if !matches!(
                    current_state,
                    RelationshipState::Proposed | RelationshipState::Active
                ) {
                    return Err(EntityReviewError::InvalidState);
                }
                (
                    RelationshipState::Retired,
                    "retire",
                    "reviewed lifecycle transition".to_string(),
                )
            }
        };

        let record = self
            .relationships
            .get_mut(&id)
            .ok_or(EntityReviewError::NotFound)?;
        record.relationship.state = new_state;
        record.relationship.version += 1;
        let updated = record.relationship.clone();

        self.activity.push(EntityActivity {
            action: action_name.into(),
            target_id: id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason,
            previous_version: Some(current_version),
            resulting_version: Some(updated.version),
            merged_into: None,
        });
        self.operations
            .insert(replay_key, updated.version.to_string());
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engrave_contracts::MapRelationDefinition;

    fn entity_input(kind: &str, proposer: &str, policy: EntityProposalPolicy) -> EntityDraftInput {
        EntityDraftInput {
            tenant_id: TenantId::new_v7(),
            area_id: AreaId::new_v7(),
            map_version_id: MapVersionId::new_v7(),
            proposer: proposer.into(),
            reason: "captured from sales call".into(),
            kind: kind.into(),
            descriptor: serde_json::json!({"name": "Acme"}),
            origin: Origin::Observed,
            policy,
            governing_kinds: vec!["account".into(), "person".into()],
        }
    }

    fn relation_def() -> MapRelationDefinition {
        MapRelationDefinition {
            key: "owns".into(),
            label: "Owns".into(),
            source_kind: "person".into(),
            target_kind: "account".into(),
            description: None,
        }
    }

    fn relationship_input(
        source: EntityId,
        target: EntityId,
        proposer: &str,
        policy: EntityProposalPolicy,
    ) -> RelationshipDraftInput {
        RelationshipDraftInput {
            tenant_id: TenantId::new_v7(),
            area_id: AreaId::new_v7(),
            map_version_id: MapVersionId::new_v7(),
            proposer: proposer.into(),
            reason: "captured from sales call".into(),
            source_entity_id: source,
            target_entity_id: target,
            relation_kind: "owns".into(),
            origin: Origin::Observed,
            policy,
            governing_relations: vec![relation_def()],
        }
    }

    fn approved_entity(store: &mut EntityStore, kind: &str, proposer: &str) -> EntityId {
        let id = EntityId::new_v7();
        store
            .propose_entity(
                id,
                entity_input(kind, proposer, EntityProposalPolicy::PersonalArea),
            )
            .unwrap();
        store
            .review_entity(id, proposer, 1, "approve", EntityReviewAction::Approve)
            .unwrap();
        id
    }

    #[test]
    fn proposing_an_entity_never_activates_it() {
        let mut store = EntityStore::default();
        let id = EntityId::new_v7();
        let entity = store
            .propose_entity(
                id,
                entity_input("account", "agent", EntityProposalPolicy::PersonalArea),
            )
            .unwrap();
        assert_eq!(entity.state, EntityState::Proposed);
        assert_eq!(store.entity(id).unwrap().state, EntityState::Proposed);
    }

    #[test]
    fn proposing_a_relationship_never_activates_it() {
        let mut store = EntityStore::default();
        let source = approved_entity(&mut store, "person", "agent");
        let target = approved_entity(&mut store, "account", "agent");
        let id = RelationshipId::new_v7();
        let relationship = store
            .propose_relationship(
                id,
                relationship_input(source, target, "agent", EntityProposalPolicy::PersonalArea),
            )
            .unwrap();
        assert_eq!(relationship.state, RelationshipState::Proposed);
        assert_eq!(
            store.relationship(id).unwrap().state,
            RelationshipState::Proposed
        );
    }

    #[test]
    fn unknown_kind_is_rejected_at_proposal_time() {
        let mut store = EntityStore::default();
        let id = EntityId::new_v7();
        assert_eq!(
            store.propose_entity(
                id,
                entity_input("unknown_kind", "agent", EntityProposalPolicy::PersonalArea)
            ),
            Err(EntityReviewError::UnknownKind)
        );
    }

    #[test]
    fn unknown_relation_kind_is_rejected() {
        let mut store = EntityStore::default();
        let source = approved_entity(&mut store, "person", "agent");
        let target = approved_entity(&mut store, "account", "agent");
        let mut input =
            relationship_input(source, target, "agent", EntityProposalPolicy::PersonalArea);
        input.relation_kind = "unknown_relation".into();
        assert_eq!(
            store.propose_relationship(RelationshipId::new_v7(), input),
            Err(EntityReviewError::UnknownRelation)
        );
    }

    #[test]
    fn dangling_entity_reference_is_rejected() {
        let mut store = EntityStore::default();
        let target = approved_entity(&mut store, "account", "agent");
        let missing_source = EntityId::new_v7();
        let input = relationship_input(
            missing_source,
            target,
            "agent",
            EntityProposalPolicy::PersonalArea,
        );
        assert_eq!(
            store.propose_relationship(RelationshipId::new_v7(), input),
            Err(EntityReviewError::DanglingEntityReference)
        );
    }

    #[test]
    fn relation_kind_mismatch_is_rejected() {
        let mut store = EntityStore::default();
        // Both entities are "account" kind, but "owns" requires source_kind
        // "person" and target_kind "account" — a mismatch.
        let source = approved_entity(&mut store, "account", "agent");
        let target = approved_entity(&mut store, "account", "agent");
        let input = relationship_input(source, target, "agent", EntityProposalPolicy::PersonalArea);
        assert_eq!(
            store.propose_relationship(RelationshipId::new_v7(), input),
            Err(EntityReviewError::KindMismatch)
        );
    }

    #[test]
    fn personal_self_approval_and_shared_independent_review_are_distinct() {
        let mut store = EntityStore::default();
        let personal_id = EntityId::new_v7();
        store
            .propose_entity(
                personal_id,
                entity_input("account", "agent", EntityProposalPolicy::PersonalArea),
            )
            .unwrap();
        assert!(store
            .review_entity(personal_id, "agent", 1, "p1", EntityReviewAction::Approve)
            .is_ok());

        let shared_id = EntityId::new_v7();
        store
            .propose_entity(
                shared_id,
                entity_input("account", "agent", EntityProposalPolicy::SharedArea),
            )
            .unwrap();
        assert_eq!(
            store.review_entity(shared_id, "agent", 1, "s1", EntityReviewAction::Approve),
            Err(EntityReviewError::IndependentReviewRequired)
        );
        assert!(store
            .review_entity(shared_id, "reviewer", 1, "s1", EntityReviewAction::Approve)
            .is_ok());
    }

    #[test]
    fn review_is_idempotent_on_replay_key() {
        let mut store = EntityStore::default();
        let id = EntityId::new_v7();
        store
            .propose_entity(
                id,
                entity_input("account", "agent", EntityProposalPolicy::PersonalArea),
            )
            .unwrap();
        let first = store
            .review_entity(id, "agent", 1, "approve", EntityReviewAction::Approve)
            .unwrap();
        let replay = store
            .review_entity(id, "agent", 1, "approve", EntityReviewAction::Approve)
            .unwrap();
        assert_eq!(first, replay);
        assert_eq!(store.entity(id).unwrap().version, 2);
        assert_eq!(
            store
                .activity()
                .iter()
                .filter(|event| event.action == "approve")
                .count(),
            1
        );
    }

    #[test]
    fn stale_expected_version_is_rejected() {
        let mut store = EntityStore::default();
        let id = EntityId::new_v7();
        store
            .propose_entity(
                id,
                entity_input("account", "agent", EntityProposalPolicy::PersonalArea),
            )
            .unwrap();
        assert_eq!(
            store.review_entity(id, "agent", 99, "approve", EntityReviewAction::Approve),
            Err(EntityReviewError::VersionConflict { current_version: 1 })
        );
    }

    #[test]
    fn retire_and_merge_are_transitions_not_deletions() {
        let mut store = EntityStore::default();
        let retiree = approved_entity(&mut store, "account", "agent");
        store
            .review_entity(retiree, "agent", 2, "retire", EntityReviewAction::Retire)
            .unwrap();
        let retired = store.entity(retiree).unwrap();
        assert_eq!(retired.state, EntityState::Retired);
        assert_eq!(retired.kind, "account");

        let source = approved_entity(&mut store, "account", "agent");
        let target = approved_entity(&mut store, "account", "agent");
        let original_descriptor = store.entity(source).unwrap().descriptor.clone();
        let original_origin = store.entity(source).unwrap().origin;
        store
            .review_entity(
                source,
                "agent",
                2,
                "merge",
                EntityReviewAction::Merge { into: target },
            )
            .unwrap();
        let merged = store.entity(source).unwrap();
        assert_eq!(merged.state, EntityState::Merged);
        assert_eq!(merged.kind, "account");
        assert_eq!(merged.descriptor, original_descriptor);
        assert_eq!(merged.origin, original_origin);
        assert_eq!(store.merged_into(source), Some(target));
        assert!(store.activity().iter().any(|event| event.action == "merge"
            && event.merged_into == Some(target.as_uuid().to_string())));
    }

    #[test]
    fn merge_is_reversible_and_identity_groups_reflect_it() {
        let mut store = EntityStore::default();
        let source = approved_entity(&mut store, "account", "agent");
        let target = approved_entity(&mut store, "account", "agent");
        let unrelated = approved_entity(&mut store, "account", "agent");

        // Before any merge: no identity groups (nothing to group).
        assert!(store.identity_groups().is_empty());

        store
            .review_entity(
                source,
                "agent",
                2,
                "merge",
                EntityReviewAction::Merge { into: target },
            )
            .unwrap();

        let groups = store.identity_groups();
        assert_eq!(groups.len(), 1);
        let group = &groups[0];
        assert_eq!(group.canonical, target);
        assert_eq!(
            group
                .members
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from([source, target])
        );
        assert!(!group.members.contains(&unrelated));

        // Reversal: Unmerge restores Active and clears the identity group,
        // without deleting or rewriting the Area-local record.
        store
            .review_entity(source, "agent", 3, "unmerge", EntityReviewAction::Unmerge)
            .unwrap();
        let restored = store.entity(source).unwrap();
        assert_eq!(restored.state, EntityState::Active);
        assert_eq!(restored.kind, "account");
        assert_eq!(store.merged_into(source), None);
        assert!(store.identity_groups().is_empty());
        assert!(store
            .activity()
            .iter()
            .any(|event| event.action == "unmerge" && event.merged_into.is_none()));
    }

    #[test]
    fn unmerge_on_a_non_merged_entity_is_rejected() {
        let mut store = EntityStore::default();
        let id = approved_entity(&mut store, "account", "agent");
        assert_eq!(
            store.review_entity(id, "agent", 2, "unmerge", EntityReviewAction::Unmerge),
            Err(EntityReviewError::InvalidState)
        );
    }

    #[test]
    fn relationship_retire_is_a_transition_not_a_deletion() {
        let mut store = EntityStore::default();
        let source = approved_entity(&mut store, "person", "agent");
        let target = approved_entity(&mut store, "account", "agent");
        let id = RelationshipId::new_v7();
        store
            .propose_relationship(
                id,
                relationship_input(source, target, "agent", EntityProposalPolicy::PersonalArea),
            )
            .unwrap();
        store
            .review_relationship(id, "agent", 1, "approve", RelationshipReviewAction::Approve)
            .unwrap();
        store
            .review_relationship(id, "agent", 2, "retire", RelationshipReviewAction::Retire)
            .unwrap();
        let retired = store.relationship(id).unwrap();
        assert_eq!(retired.state, RelationshipState::Retired);
        assert_eq!(retired.relation_kind, "owns");
    }

    #[test]
    fn relationship_shared_area_requires_independent_reviewer() {
        let mut store = EntityStore::default();
        let source = approved_entity(&mut store, "person", "agent");
        let target = approved_entity(&mut store, "account", "agent");
        let id = RelationshipId::new_v7();
        store
            .propose_relationship(
                id,
                relationship_input(source, target, "agent", EntityProposalPolicy::SharedArea),
            )
            .unwrap();
        assert_eq!(
            store.review_relationship(id, "agent", 1, "a1", RelationshipReviewAction::Approve),
            Err(EntityReviewError::IndependentReviewRequired)
        );
        assert!(store
            .review_relationship(id, "reviewer", 1, "a1", RelationshipReviewAction::Approve)
            .is_ok());
    }
}
