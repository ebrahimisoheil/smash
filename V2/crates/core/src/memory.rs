//! Proposal-first Memory admission and lifecycle semantics.
//!
//! This module is deliberately storage-free.  PostgreSQL adapters persist the
//! same state transitions, while these types make the safety invariants
//! executable in contract tests.

use engrave_contracts::{MemoryId, MemoryState, MemoryVersionId, ProposalId, ProposalState};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionPolicy {
    PersonalArea,
    SharedArea,
    CrossMap,
}

impl AdmissionPolicy {
    fn independent_reviewer_required(self) -> bool {
        !matches!(self, Self::PersonalArea)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProposalInput {
    pub proposer: String,
    pub claim: String,
    pub reason: String,
    pub scope: String,
    pub applies_when: String,
    pub evidence: Vec<String>,
    pub policy: AdmissionPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Proposal {
    pub id: ProposalId,
    pub input: ProposalInput,
    pub state: ProposalState,
    pub version: u64,
    pub duplicate_candidates: Vec<MemoryId>,
    pub contradiction_candidates: Vec<MemoryId>,
    pub rejection_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Memory {
    pub id: MemoryId,
    pub version_id: MemoryVersionId,
    pub claim: String,
    pub reason: String,
    pub scope: String,
    pub applies_when: String,
    pub evidence: Vec<String>,
    pub state: MemoryState,
    pub version: u64,
    pub predecessor: Option<MemoryId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Activity {
    pub action: String,
    pub target_id: String,
    pub actor: String,
    pub reason: String,
    pub evidence: Vec<String>,
    pub previous_version: Option<u64>,
    pub resulting_version: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAction {
    Approve,
    Reject { reason: String },
    Edit(ProposalInput),
    Archive,
    Restore,
    Expire,
    Supersede { replacement: ProposalInput },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewError {
    NotFound,
    InvalidState,
    VersionConflict { current_version: u64 },
    IdempotencyConflict,
    IndependentReviewRequired,
}

#[derive(Default)]
pub struct MemoryStore {
    proposals: BTreeMap<ProposalId, Proposal>,
    memories: BTreeMap<MemoryId, Memory>,
    operations: BTreeMap<String, String>,
    activity: Vec<Activity>,
}

impl MemoryStore {
    /// Creates a reviewable proposal. This function never creates an active
    /// Memory, even for Personal-Area policy.
    pub fn propose(&mut self, id: ProposalId, input: ProposalInput) -> Proposal {
        let normalized = normalize(&input.claim);
        let mut duplicates = BTreeSet::new();
        let mut contradictions = BTreeSet::new();
        for memory in self
            .memories
            .values()
            .filter(|m| m.state == MemoryState::Active)
        {
            if memory.scope == input.scope && memory.applies_when == input.applies_when {
                if normalize(&memory.claim) == normalized {
                    duplicates.insert(memory.id);
                } else if contradicts(&memory.claim, &input.claim) {
                    contradictions.insert(memory.id);
                }
            }
        }
        let proposal = Proposal {
            id,
            input: input.clone(),
            state: ProposalState::Pending,
            version: 1,
            duplicate_candidates: duplicates.into_iter().collect(),
            contradiction_candidates: contradictions.into_iter().collect(),
            rejection_reason: None,
        };
        self.activity.push(Activity {
            action: "create".into(),
            target_id: id.as_uuid().to_string(),
            actor: input.proposer,
            reason: input.reason,
            evidence: input.evidence,
            previous_version: None,
            resulting_version: Some(1),
        });
        self.proposals.insert(id, proposal.clone());
        proposal
    }

    pub fn proposal(&self, id: ProposalId) -> Option<&Proposal> {
        self.proposals.get(&id)
    }
    pub fn memory(&self, id: MemoryId) -> Option<&Memory> {
        self.memories.get(&id)
    }
    pub fn activity(&self) -> &[Activity] {
        &self.activity
    }

    pub fn review(
        &mut self,
        proposal_id: ProposalId,
        reviewer: &str,
        expected_version: u64,
        idempotency_key: &str,
        action: ReviewAction,
    ) -> Result<Option<MemoryId>, ReviewError> {
        let replay_key = format!("proposal:{}:{}", proposal_id.as_uuid(), idempotency_key);
        if let Some(result) = self.operations.get(&replay_key) {
            return Ok(result.parse().ok().map(MemoryId::new));
        }
        let proposal = self
            .proposals
            .get(&proposal_id)
            .ok_or(ReviewError::NotFound)?;
        if proposal.version != expected_version {
            return Err(ReviewError::VersionConflict {
                current_version: proposal.version,
            });
        }
        if proposal.state != ProposalState::Pending {
            return Err(ReviewError::InvalidState);
        }
        if proposal.input.policy.independent_reviewer_required()
            && proposal.input.proposer == reviewer
        {
            return Err(ReviewError::IndependentReviewRequired);
        }
        let input = proposal.input.clone();
        let result = match action {
            ReviewAction::Approve => Some(self.activate(proposal_id, reviewer, input)?),
            ReviewAction::Reject { reason } => {
                self.reject(proposal_id, reviewer, reason)?;
                None
            }
            ReviewAction::Edit(edited) => {
                self.edit(proposal_id, reviewer, edited)?;
                None
            }
            _ => return Err(ReviewError::InvalidState),
        };
        self.operations.insert(
            replay_key,
            result
                .map(|id| id.as_uuid().to_string())
                .unwrap_or_default(),
        );
        Ok(result)
    }

    pub fn mutate_memory(
        &mut self,
        id: MemoryId,
        reviewer: &str,
        expected_version: u64,
        key: &str,
        action: ReviewAction,
    ) -> Result<MemoryId, ReviewError> {
        let replay_key = format!("memory:{}:{}", id.as_uuid(), key);
        if let Some(result) = self.operations.get(&replay_key) {
            return result
                .parse()
                .map(MemoryId::new)
                .map_err(|_| ReviewError::InvalidState);
        }
        let current = self.memories.get(&id).ok_or(ReviewError::NotFound)?.clone();
        if current.version != expected_version {
            return Err(ReviewError::VersionConflict {
                current_version: current.version,
            });
        }
        let new_state = match action {
            ReviewAction::Archive => MemoryState::Archived,
            ReviewAction::Restore => MemoryState::Active,
            ReviewAction::Expire => MemoryState::Expired,
            _ => return Err(ReviewError::InvalidState),
        };
        let mut updated = current.clone();
        updated.state = new_state;
        updated.version += 1;
        self.memories.insert(id, updated.clone());
        self.activity.push(Activity {
            action: match new_state {
                MemoryState::Archived => "archive",
                MemoryState::Active => "restore",
                MemoryState::Expired => "expire",
                _ => "update",
            }
            .into(),
            target_id: id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason: "reviewed lifecycle transition".into(),
            evidence: updated.evidence.clone(),
            previous_version: Some(current.version),
            resulting_version: Some(updated.version),
        });
        self.operations.insert(replay_key, id.as_uuid().to_string());
        Ok(id)
    }

    pub fn supersede(
        &mut self,
        id: MemoryId,
        reviewer: &str,
        expected_version: u64,
        key: &str,
        replacement: ProposalInput,
        new_id: MemoryId,
    ) -> Result<MemoryId, ReviewError> {
        let current = self.memories.get(&id).ok_or(ReviewError::NotFound)?.clone();
        if current.version != expected_version {
            return Err(ReviewError::VersionConflict {
                current_version: current.version,
            });
        }
        let proposal = self.propose(ProposalId::new_v7(), replacement);
        let activated = self
            .review(proposal.id, reviewer, 1, key, ReviewAction::Approve)?
            .ok_or(ReviewError::InvalidState)?;
        let mut old = current;
        old.state = MemoryState::Superseded;
        old.version += 1;
        self.memories.insert(id, old.clone());
        let mut new_memory = self
            .memories
            .remove(&activated)
            .ok_or(ReviewError::NotFound)?;
        new_memory.id = new_id;
        new_memory.predecessor = Some(id);
        self.memories.insert(new_id, new_memory);
        self.activity.push(Activity {
            action: "supersede".into(),
            target_id: new_id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason: "replacement approved with explicit lineage".into(),
            evidence: Vec::new(),
            previous_version: Some(old.version - 1),
            resulting_version: Some(old.version),
        });
        Ok(new_id)
    }

    fn activate(
        &mut self,
        id: ProposalId,
        reviewer: &str,
        input: ProposalInput,
    ) -> Result<MemoryId, ReviewError> {
        let memory_id = MemoryId::new_v7();
        let memory = Memory {
            id: memory_id,
            version_id: MemoryVersionId::new_v7(),
            claim: input.claim.clone(),
            reason: input.reason.clone(),
            scope: input.scope,
            applies_when: input.applies_when,
            evidence: input.evidence.clone(),
            state: MemoryState::Active,
            version: 1,
            predecessor: None,
        };
        self.memories.insert(memory_id, memory);
        let proposal = self.proposals.get_mut(&id).ok_or(ReviewError::NotFound)?;
        proposal.state = ProposalState::Approved;
        proposal.version += 1;
        self.activity.push(Activity {
            action: "approve".into(),
            target_id: memory_id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason: input.reason,
            evidence: input.evidence,
            previous_version: Some(1),
            resulting_version: Some(1),
        });
        Ok(memory_id)
    }

    fn reject(
        &mut self,
        id: ProposalId,
        reviewer: &str,
        reason: String,
    ) -> Result<(), ReviewError> {
        let proposal = self.proposals.get_mut(&id).ok_or(ReviewError::NotFound)?;
        proposal.state = ProposalState::Rejected;
        proposal.rejection_reason = Some(reason.clone());
        proposal.version += 1;
        self.activity.push(Activity {
            action: "reject".into(),
            target_id: id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason,
            evidence: Vec::new(),
            previous_version: Some(proposal.version - 1),
            resulting_version: Some(proposal.version),
        });
        Ok(())
    }

    fn edit(
        &mut self,
        id: ProposalId,
        reviewer: &str,
        edited: ProposalInput,
    ) -> Result<(), ReviewError> {
        let proposal = self.proposals.get_mut(&id).ok_or(ReviewError::NotFound)?;
        proposal.input = edited.clone();
        proposal.version += 1;
        self.activity.push(Activity {
            action: "update".into(),
            target_id: id.as_uuid().to_string(),
            actor: reviewer.into(),
            reason: "proposal edited before admission".into(),
            evidence: edited.evidence,
            previous_version: Some(proposal.version - 1),
            resulting_version: Some(proposal.version),
        });
        Ok(())
    }
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn contradicts(left: &str, right: &str) -> bool {
    let left = normalize(left);
    let right = normalize(right);
    let (positive, negative) = if let Some(stripped) = left.strip_prefix("not ") {
        (right, stripped.to_owned())
    } else if let Some(stripped) = right.strip_prefix("not ") {
        (left, stripped.to_owned())
    } else {
        return false;
    };
    positive == negative
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(claim: &str, policy: AdmissionPolicy) -> ProposalInput {
        ProposalInput {
            proposer: "agent".into(),
            claim: claim.into(),
            reason: "explicit reason".into(),
            scope: "area:personal".into(),
            applies_when: "always".into(),
            evidence: vec!["chunk:1#line=1".into()],
            policy,
        }
    }

    #[test]
    fn proposal_first_never_activates_on_capture() {
        let mut store = MemoryStore::default();
        let proposal = store.propose(
            ProposalId::new_v7(),
            input("Keep concise", AdmissionPolicy::PersonalArea),
        );
        assert_eq!(proposal.state, ProposalState::Pending);
        assert!(store.memories.is_empty());
    }

    #[test]
    fn personal_confirmation_and_shared_independent_review_are_distinct() {
        let mut store = MemoryStore::default();
        let personal = store.propose(
            ProposalId::new_v7(),
            input("Keep concise", AdmissionPolicy::PersonalArea),
        );
        assert!(store
            .review(personal.id, "agent", 1, "p1", ReviewAction::Approve)
            .unwrap()
            .is_some());
        let shared = store.propose(
            ProposalId::new_v7(),
            input("Share only approved facts", AdmissionPolicy::SharedArea),
        );
        assert_eq!(
            store.review(shared.id, "agent", 1, "s1", ReviewAction::Approve),
            Err(ReviewError::IndependentReviewRequired)
        );
        assert!(store
            .review(shared.id, "reviewer", 1, "s1", ReviewAction::Approve)
            .unwrap()
            .is_some());
    }

    #[test]
    fn duplicate_contradiction_and_review_concurrency_are_visible() {
        let mut store = MemoryStore::default();
        let first = store.propose(
            ProposalId::new_v7(),
            input("not external", AdmissionPolicy::PersonalArea),
        );
        let memory_id = store
            .review(first.id, "agent", 1, "a", ReviewAction::Approve)
            .unwrap()
            .unwrap();
        let duplicate = store.propose(
            ProposalId::new_v7(),
            input("NOT external", AdmissionPolicy::PersonalArea),
        );
        assert_eq!(duplicate.duplicate_candidates, vec![memory_id]);
        let contradiction = store.propose(
            ProposalId::new_v7(),
            input("external", AdmissionPolicy::PersonalArea),
        );
        assert_eq!(contradiction.contradiction_candidates, vec![memory_id]);
        assert_eq!(
            store.review(
                contradiction.id,
                "reviewer",
                99,
                "b",
                ReviewAction::Reject {
                    reason: "stale view".into()
                }
            ),
            Err(ReviewError::VersionConflict { current_version: 1 })
        );
    }

    #[test]
    fn lifecycle_is_idempotent_and_explainable() {
        let mut store = MemoryStore::default();
        let proposal = store.propose(
            ProposalId::new_v7(),
            input("Use UTC", AdmissionPolicy::PersonalArea),
        );
        let id = store
            .review(proposal.id, "agent", 1, "approve", ReviewAction::Approve)
            .unwrap()
            .unwrap();
        let replay = store
            .review(proposal.id, "agent", 1, "approve", ReviewAction::Approve)
            .unwrap()
            .unwrap();
        assert_eq!(id, replay);
        store
            .mutate_memory(id, "reviewer", 1, "archive", ReviewAction::Archive)
            .unwrap();
        store
            .mutate_memory(id, "reviewer", 2, "restore", ReviewAction::Restore)
            .unwrap();
        store
            .mutate_memory(id, "reviewer", 3, "expire", ReviewAction::Expire)
            .unwrap();
        assert_eq!(store.memory(id).unwrap().state, MemoryState::Expired);
        assert!(store
            .activity()
            .iter()
            .any(|event| event.action == "approve" && !event.evidence.is_empty()));
    }
}
