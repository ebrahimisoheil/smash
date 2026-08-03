//! Cross-Map mapping approval/expiry/revocation lifecycle.
//!
//! This module is deliberately storage-free, mirroring `memory.rs` and
//! `maps.rs`: it makes the Cross-Map governance invariants executable in
//! contract tests before any PostgreSQL adapter exists.
//!
//! Cross-Map mappings are inherently cross-authority: a mapping asserts a
//! semantic relation (`CrossMapRelation`) between two different Areas, so
//! unlike Memory/Map's Personal-Area self-approval carve-out, there is no
//! self-approval path here at all. `propose` always leaves a mapping in
//! `CrossMapMappingState::Proposed`; only an explicit, independently
//! reviewed `review(..., CrossMapReviewAction::Approve)` call can activate
//! it.
//!
//! ## Expiry model
//!
//! `CrossMapMapping` (the shared contract type) has no expiry field, so this
//! store keeps the optional expiry timestamp as its own bookkeeping data
//! alongside the mapping, set at `Approve` time and left untouched by every
//! other transition. `is_traversable` is the single source of truth for
//! whether a mapping may be used to generate a Cross-Map candidate: it is a
//! pure function of `(state, now, expiry)` and never mutates anything. In
//! particular, a mapping whose `state` field still literally reads
//! `Approved` past its expiry is deliberately *not* lazily rewritten to
//! `Expired` by a read — `is_traversable` returning `false` is what future
//! retrieval/graph code must call before generating any candidate, and it is
//! correct on every read regardless of whether anyone has "touched" the
//! mapping since it expired. This keeps the predicate storage-free and free
//! of read-time side effects, at the cost of the stored `state` occasionally
//! lagging reality until an explicit `Revoke` (or a future explicit `Expire`
//! action, not required by this session) catches it up.

use engrave_contracts::{
    AreaId, CrossMapMapping, CrossMapMappingId, CrossMapMappingState, CrossMapRelation,
    MapVersionId, TenantId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use time::OffsetDateTime;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CrossMapProposalInput {
    pub tenant_id: TenantId,
    pub source_area_id: AreaId,
    pub target_area_id: AreaId,
    pub source_map_version_id: MapVersionId,
    pub target_map_version_id: MapVersionId,
    pub relation: CrossMapRelation,
    pub rationale: String,
    pub proposer: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossMapReviewAction {
    Approve { expires_at: Option<OffsetDateTime> },
    Reject { reason: String },
    Revoke { reason: String },
    Block { reason: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossMapReviewError {
    NotFound,
    InvalidState,
    VersionConflict { current_version: u64 },
    IndependentReviewRequired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossMapActivity {
    pub action: String,
    pub target_id: String,
    pub actor: String,
    pub reason: String,
    pub previous_version: Option<u64>,
    pub resulting_version: Option<u64>,
}

/// Internal bookkeeping kept next to each mapping: who proposed it (needed
/// to enforce independent review, since the shared `CrossMapMapping`
/// contract type has no proposer field) and the optional expiry set at
/// approval time.
#[derive(Clone, Debug, Eq, PartialEq)]
struct MappingRecord {
    mapping: CrossMapMapping,
    proposer: String,
    expires_at: Option<OffsetDateTime>,
}

/// Pure predicate: is a mapping usable to generate a Cross-Map candidate
/// right now? `true` iff `state == Approved` and either no expiry is
/// configured or `now` has not yet reached it. Every other state
/// (`Proposed`, `Rejected`, `Blocked`, `Expired`, `Revoked`, `Superseded`)
/// always yields `false`. This is the check a future retrieval/graph
/// session must call before generating any candidate — never only after.
pub fn is_traversable(
    mapping: &CrossMapMapping,
    now: OffsetDateTime,
    expiry: Option<OffsetDateTime>,
) -> bool {
    mapping.state == CrossMapMappingState::Approved
        && expiry.is_none_or(|expires_at| now < expires_at)
}

#[derive(Default)]
pub struct CrossMapStore {
    mappings: BTreeMap<CrossMapMappingId, MappingRecord>,
    operations: BTreeMap<String, CrossMapMapping>,
    activity: Vec<CrossMapActivity>,
}

impl CrossMapStore {
    /// Creates a mapping in `Proposed` state. This function never activates
    /// a mapping, regardless of the caller's authority — activation always
    /// goes through `review`.
    pub fn propose(
        &mut self,
        id: CrossMapMappingId,
        input: CrossMapProposalInput,
    ) -> CrossMapMapping {
        let mapping = CrossMapMapping {
            cross_map_mapping_id: id,
            tenant_id: input.tenant_id,
            source_area_id: input.source_area_id,
            target_area_id: input.target_area_id,
            source_map_version_id: input.source_map_version_id,
            target_map_version_id: input.target_map_version_id,
            relation: input.relation,
            state: CrossMapMappingState::Proposed,
            rationale: input.rationale.clone(),
            version: 1,
        };
        self.activity.push(CrossMapActivity {
            action: "propose".into(),
            target_id: id.as_uuid().to_string(),
            actor: input.proposer.clone(),
            reason: input.rationale,
            previous_version: None,
            resulting_version: Some(1),
        });
        self.mappings.insert(
            id,
            MappingRecord {
                mapping: mapping.clone(),
                proposer: input.proposer,
                expires_at: None,
            },
        );
        mapping
    }

    pub fn mapping(&self, id: CrossMapMappingId) -> Option<&CrossMapMapping> {
        self.mappings.get(&id).map(|record| &record.mapping)
    }

    pub fn activity(&self) -> &[CrossMapActivity] {
        &self.activity
    }

    /// Returns `false` (never an error) for an unknown/missing id, since
    /// "not traversable" is the safe answer for a mapping that does not
    /// exist in this store's view.
    pub fn is_traversable(&self, id: CrossMapMappingId, now: OffsetDateTime) -> bool {
        match self.mappings.get(&id) {
            Some(record) => is_traversable(&record.mapping, now, record.expires_at),
            None => false,
        }
    }

    /// Governed, idempotent, version-checked lifecycle transition. Replaying
    /// the same `(id, idempotency_key)` pair returns the exact result of the
    /// original successful call without re-applying the transition.
    pub fn review(
        &mut self,
        id: CrossMapMappingId,
        reviewer: &str,
        expected_version: u64,
        idempotency_key: &str,
        action: CrossMapReviewAction,
    ) -> Result<CrossMapMapping, CrossMapReviewError> {
        let replay_key = format!("cross-map:{}:{}", id.as_uuid(), idempotency_key);
        if let Some(result) = self.operations.get(&replay_key) {
            return Ok(result.clone());
        }

        let record = self
            .mappings
            .get(&id)
            .ok_or(CrossMapReviewError::NotFound)?;
        if record.mapping.version != expected_version {
            return Err(CrossMapReviewError::VersionConflict {
                current_version: record.mapping.version,
            });
        }

        // Blocked is permanently terminal: no further review action may
        // move a Blocked mapping anywhere else, including re-blocking it.
        if record.mapping.state == CrossMapMappingState::Blocked {
            return Err(CrossMapReviewError::InvalidState);
        }

        match &action {
            CrossMapReviewAction::Approve { .. } => {
                if record.mapping.state != CrossMapMappingState::Proposed {
                    return Err(CrossMapReviewError::InvalidState);
                }
                // Cross-Map spans two Areas: there is no self-approval path
                // at all, unlike Memory/Map's Personal-Area carve-out.
                if record.proposer == reviewer {
                    return Err(CrossMapReviewError::IndependentReviewRequired);
                }
            }
            CrossMapReviewAction::Reject { .. } => {
                if record.mapping.state != CrossMapMappingState::Proposed {
                    return Err(CrossMapReviewError::InvalidState);
                }
            }
            CrossMapReviewAction::Revoke { .. } => {
                if record.mapping.state != CrossMapMappingState::Approved {
                    return Err(CrossMapReviewError::InvalidState);
                }
            }
            CrossMapReviewAction::Block { .. } => {
                if !matches!(
                    record.mapping.state,
                    CrossMapMappingState::Proposed | CrossMapMappingState::Approved
                ) {
                    return Err(CrossMapReviewError::InvalidState);
                }
            }
        }

        let result = match action {
            CrossMapReviewAction::Approve { expires_at } => self.transition(
                id,
                reviewer,
                CrossMapMappingState::Approved,
                "approve",
                "independent review approved",
                expires_at,
            )?,
            CrossMapReviewAction::Reject { reason } => self.transition(
                id,
                reviewer,
                CrossMapMappingState::Rejected,
                "reject",
                reason,
                None,
            )?,
            CrossMapReviewAction::Revoke { reason } => self.transition(
                id,
                reviewer,
                CrossMapMappingState::Revoked,
                "revoke",
                reason,
                None,
            )?,
            CrossMapReviewAction::Block { reason } => self.transition(
                id,
                reviewer,
                CrossMapMappingState::Blocked,
                "block",
                reason,
                None,
            )?,
        };

        self.operations.insert(replay_key, result.clone());
        Ok(result)
    }

    /// Applies a state transition, bumping `version` and recording lineage.
    /// `source_area_id`, `target_area_id`, `source_map_version_id`,
    /// `target_map_version_id`, and `relation` are never touched here.
    /// `expires_at` is only meaningful (and only passed non-`None`) on
    /// `Approve`; every other transition leaves the stored expiry as-is.
    fn transition(
        &mut self,
        id: CrossMapMappingId,
        reviewer: &str,
        new_state: CrossMapMappingState,
        action: &str,
        reason: impl Into<String>,
        expires_at: Option<OffsetDateTime>,
    ) -> Result<CrossMapMapping, CrossMapReviewError> {
        let record = self
            .mappings
            .get_mut(&id)
            .ok_or(CrossMapReviewError::NotFound)?;
        let previous_version = record.mapping.version;
        record.mapping.state = new_state;
        record.mapping.version += 1;
        if new_state == CrossMapMappingState::Approved {
            record.expires_at = expires_at;
        }
        let updated = record.mapping.clone();
        self.activity.push(CrossMapActivity {
            action: action.into(),
            target_id: id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason: reason.into(),
            previous_version: Some(previous_version),
            resulting_version: Some(updated.version),
        });
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn input(proposer: &str) -> CrossMapProposalInput {
        CrossMapProposalInput {
            tenant_id: TenantId::new_v7(),
            source_area_id: AreaId::new_v7(),
            target_area_id: AreaId::new_v7(),
            source_map_version_id: MapVersionId::new_v7(),
            target_map_version_id: MapVersionId::new_v7(),
            relation: CrossMapRelation::RelatedTo,
            rationale: "shared account concept".into(),
            proposer: proposer.into(),
        }
    }

    fn approved_mapping(state: CrossMapMappingState) -> CrossMapMapping {
        CrossMapMapping {
            cross_map_mapping_id: CrossMapMappingId::new_v7(),
            tenant_id: TenantId::new_v7(),
            source_area_id: AreaId::new_v7(),
            target_area_id: AreaId::new_v7(),
            source_map_version_id: MapVersionId::new_v7(),
            target_map_version_id: MapVersionId::new_v7(),
            relation: CrossMapRelation::RelatedTo,
            state,
            rationale: "test".into(),
            version: 1,
        }
    }

    #[test]
    fn propose_never_auto_activates() {
        let mut store = CrossMapStore::default();
        let mapping = store.propose(CrossMapMappingId::new_v7(), input("agent"));
        assert_eq!(mapping.state, CrossMapMappingState::Proposed);
    }

    // --- Exhaustive is_traversable state matrix -----------------------

    #[test]
    fn proposed_is_not_traversable() {
        let mapping = approved_mapping(CrossMapMappingState::Proposed);
        assert!(!is_traversable(&mapping, OffsetDateTime::now_utc(), None));
    }

    #[test]
    fn approved_with_no_expiry_is_traversable() {
        let mapping = approved_mapping(CrossMapMappingState::Approved);
        assert!(is_traversable(&mapping, OffsetDateTime::now_utc(), None));
    }

    #[test]
    fn approved_with_future_expiry_is_traversable() {
        let mapping = approved_mapping(CrossMapMappingState::Approved);
        let now = OffsetDateTime::now_utc();
        assert!(is_traversable(
            &mapping,
            now,
            Some(now + Duration::hours(1))
        ));
    }

    #[test]
    fn approved_with_past_expiry_is_not_traversable() {
        // Critical stale-approval test: state still literally reads
        // Approved, but the expiry has passed.
        let mapping = approved_mapping(CrossMapMappingState::Approved);
        let now = OffsetDateTime::now_utc();
        assert_eq!(mapping.state, CrossMapMappingState::Approved);
        assert!(!is_traversable(
            &mapping,
            now,
            Some(now - Duration::hours(1))
        ));
    }

    #[test]
    fn rejected_is_not_traversable() {
        let mapping = approved_mapping(CrossMapMappingState::Rejected);
        assert!(!is_traversable(&mapping, OffsetDateTime::now_utc(), None));
    }

    #[test]
    fn blocked_is_not_traversable() {
        let mapping = approved_mapping(CrossMapMappingState::Blocked);
        assert!(!is_traversable(&mapping, OffsetDateTime::now_utc(), None));
    }

    #[test]
    fn expired_is_not_traversable() {
        let mapping = approved_mapping(CrossMapMappingState::Expired);
        assert!(!is_traversable(&mapping, OffsetDateTime::now_utc(), None));
    }

    #[test]
    fn revoked_is_not_traversable() {
        let mapping = approved_mapping(CrossMapMappingState::Revoked);
        assert!(!is_traversable(&mapping, OffsetDateTime::now_utc(), None));
    }

    #[test]
    fn superseded_is_not_traversable() {
        let mapping = approved_mapping(CrossMapMappingState::Superseded);
        assert!(!is_traversable(&mapping, OffsetDateTime::now_utc(), None));
    }

    // --- Store-level review lifecycle ----------------------------------

    #[test]
    fn self_approval_is_rejected_independent_reviewer_succeeds() {
        let mut store = CrossMapStore::default();
        let mapping = store.propose(CrossMapMappingId::new_v7(), input("agent"));
        assert_eq!(
            store.review(
                mapping.cross_map_mapping_id,
                "agent",
                1,
                "approve-1",
                CrossMapReviewAction::Approve { expires_at: None },
            ),
            Err(CrossMapReviewError::IndependentReviewRequired)
        );
        let approved = store
            .review(
                mapping.cross_map_mapping_id,
                "reviewer",
                1,
                "approve-1",
                CrossMapReviewAction::Approve { expires_at: None },
            )
            .unwrap();
        assert_eq!(approved.state, CrossMapMappingState::Approved);
    }

    #[test]
    fn blocked_is_terminal_against_every_other_action() {
        let mut store = CrossMapStore::default();
        let mapping = store.propose(CrossMapMappingId::new_v7(), input("agent"));
        let id = mapping.cross_map_mapping_id;
        let blocked = store
            .review(
                id,
                "reviewer",
                1,
                "block-1",
                CrossMapReviewAction::Block {
                    reason: "policy violation".into(),
                },
            )
            .unwrap();
        assert_eq!(blocked.state, CrossMapMappingState::Blocked);
        assert_eq!(blocked.version, 2);

        assert_eq!(
            store.review(
                id,
                "reviewer",
                2,
                "approve-after-block",
                CrossMapReviewAction::Approve { expires_at: None },
            ),
            Err(CrossMapReviewError::InvalidState)
        );
        assert_eq!(
            store.review(
                id,
                "reviewer",
                2,
                "reject-after-block",
                CrossMapReviewAction::Reject {
                    reason: "too late".into()
                },
            ),
            Err(CrossMapReviewError::InvalidState)
        );
        assert_eq!(
            store.review(
                id,
                "reviewer",
                2,
                "revoke-after-block",
                CrossMapReviewAction::Revoke {
                    reason: "too late".into()
                },
            ),
            Err(CrossMapReviewError::InvalidState)
        );
        assert_eq!(
            store.review(
                id,
                "reviewer",
                2,
                "reblock",
                CrossMapReviewAction::Block {
                    reason: "again".into()
                },
            ),
            Err(CrossMapReviewError::InvalidState)
        );
    }

    #[test]
    fn revoke_is_idempotent_by_replay_key() {
        let mut store = CrossMapStore::default();
        let mapping = store.propose(CrossMapMappingId::new_v7(), input("agent"));
        let id = mapping.cross_map_mapping_id;
        store
            .review(
                id,
                "reviewer",
                1,
                "approve-1",
                CrossMapReviewAction::Approve { expires_at: None },
            )
            .unwrap();

        let first = store
            .review(
                id,
                "reviewer",
                2,
                "revoke-1",
                CrossMapReviewAction::Revoke {
                    reason: "no longer valid".into(),
                },
            )
            .unwrap();
        assert_eq!(first.state, CrossMapMappingState::Revoked);
        assert_eq!(first.version, 3);

        // Replaying the same idempotency key must return the same result
        // without double-applying (i.e. without bumping the version again
        // or erroring on a now-stale expected_version).
        let replay = store
            .review(
                id,
                "reviewer",
                2,
                "revoke-1",
                CrossMapReviewAction::Revoke {
                    reason: "no longer valid".into(),
                },
            )
            .unwrap();
        assert_eq!(replay, first);
        assert_eq!(store.mapping(id).unwrap().version, 3);
    }

    #[test]
    fn stale_expected_version_is_a_version_conflict() {
        let mut store = CrossMapStore::default();
        let mapping = store.propose(CrossMapMappingId::new_v7(), input("agent"));
        assert_eq!(
            store.review(
                mapping.cross_map_mapping_id,
                "reviewer",
                99,
                "approve-1",
                CrossMapReviewAction::Approve { expires_at: None },
            ),
            Err(CrossMapReviewError::VersionConflict { current_version: 1 })
        );
    }

    #[test]
    fn paths_and_relation_survive_propose_approve_revoke() {
        let mut store = CrossMapStore::default();
        let input = input("agent");
        let source_area_id = input.source_area_id;
        let target_area_id = input.target_area_id;
        let source_map_version_id = input.source_map_version_id;
        let target_map_version_id = input.target_map_version_id;
        let relation = input.relation;

        let proposed = store.propose(CrossMapMappingId::new_v7(), input);
        let id = proposed.cross_map_mapping_id;
        assert_eq!(proposed.source_area_id, source_area_id);
        assert_eq!(proposed.target_area_id, target_area_id);
        assert_eq!(proposed.source_map_version_id, source_map_version_id);
        assert_eq!(proposed.target_map_version_id, target_map_version_id);
        assert_eq!(proposed.relation, relation);

        let approved = store
            .review(
                id,
                "reviewer",
                1,
                "approve-1",
                CrossMapReviewAction::Approve { expires_at: None },
            )
            .unwrap();
        assert_eq!(approved.source_area_id, source_area_id);
        assert_eq!(approved.target_area_id, target_area_id);
        assert_eq!(approved.source_map_version_id, source_map_version_id);
        assert_eq!(approved.target_map_version_id, target_map_version_id);
        assert_eq!(approved.relation, relation);

        let revoked = store
            .review(
                id,
                "reviewer",
                2,
                "revoke-1",
                CrossMapReviewAction::Revoke {
                    reason: "path changed upstream".into(),
                },
            )
            .unwrap();
        assert_eq!(revoked.source_area_id, source_area_id);
        assert_eq!(revoked.target_area_id, target_area_id);
        assert_eq!(revoked.source_map_version_id, source_map_version_id);
        assert_eq!(revoked.target_map_version_id, target_map_version_id);
        assert_eq!(revoked.relation, relation);
        assert_eq!(revoked.state, CrossMapMappingState::Revoked);
    }

    #[test]
    fn unknown_mapping_is_not_traversable_and_review_reports_not_found() {
        let store = CrossMapStore::default();
        let unknown = CrossMapMappingId::new_v7();
        assert!(!store.is_traversable(unknown, OffsetDateTime::now_utc()));
    }

    #[test]
    fn review_on_unknown_mapping_is_not_found() {
        let mut store = CrossMapStore::default();
        let unknown = CrossMapMappingId::new_v7();
        assert_eq!(
            store.review(
                unknown,
                "reviewer",
                1,
                "approve-1",
                CrossMapReviewAction::Approve { expires_at: None },
            ),
            Err(CrossMapReviewError::NotFound)
        );
    }
}
