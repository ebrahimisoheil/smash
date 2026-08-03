//! Shared domain contracts for ENGRAVE V2.
//!
//! This crate intentionally contains wire/domain vocabulary only. It has no
//! web framework, database driver, or async-runtime dependency.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Deserialize,
            Eq,
            Hash,
            Ord,
            PartialEq,
            PartialOrd,
            Serialize,
            ToSchema,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new(value: Uuid) -> Self {
                Self(value)
            }
            pub fn new_v7() -> Self {
                Self(Uuid::now_v7())
            }
            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

opaque_id!(TenantId);
opaque_id!(ActorId);
opaque_id!(MembershipId);
opaque_id!(RoleId);
opaque_id!(AgentIdentityId);
opaque_id!(AreaId);
opaque_id!(AreaGrantId);
opaque_id!(PlacementId);
opaque_id!(SourceId);
opaque_id!(SourceVersionId);
opaque_id!(ArtifactId);
opaque_id!(ChunkId);
opaque_id!(EntityId);
opaque_id!(RelationshipId);
opaque_id!(MapVersionId);
opaque_id!(CrossMapMappingId);
opaque_id!(MemoryId);
opaque_id!(MemoryVersionId);
opaque_id!(EvidenceLinkId);
opaque_id!(ProposalId);
opaque_id!(RuleId);
opaque_id!(RuleVersionId);
opaque_id!(EventId);
opaque_id!(OperationId);
opaque_id!(AiRunId);
opaque_id!(DecisionEnvelopeId);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TenantState {
    Provisioning,
    Active,
    Suspended,
    Deleting,
    Deleted,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorState {
    Active,
    Disabled,
    Deleted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MembershipState {
    Invited,
    Active,
    Suspended,
    Removed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoleState {
    Draft,
    Active,
    Retired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentIdentityState {
    Registered,
    Active,
    Rotated,
    Revoked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AreaState {
    Provisioning,
    Active,
    Archived,
    Deleted,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AreaGrantState {
    Pending,
    Active,
    Expired,
    Revoked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlacementState {
    Planned,
    Provisioning,
    Active,
    Draining,
    Migrated,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceState {
    Uploaded,
    Verified,
    Queued,
    Extracting,
    Chunking,
    Indexing,
    Proposing,
    Ready,
    PartiallyReady,
    Failed,
    Quarantined,
    Deleted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceVersionState {
    Uploaded,
    Verified,
    Current,
    Superseded,
    Quarantined,
    Deleted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactState {
    Created,
    Available,
    Stale,
    Failed,
    Deleted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChunkState {
    Created,
    Indexed,
    Stale,
    Deleted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityState {
    Proposed,
    Active,
    Merged,
    Retired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipState {
    Proposed,
    Active,
    Superseded,
    Rejected,
    Retired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MapState {
    Draft,
    Published,
    Retired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryState {
    Proposed,
    Active,
    Superseded,
    Expired,
    Archived,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryVersionState {
    Draft,
    Current,
    Superseded,
    Rejected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLinkState {
    Proposed,
    Attached,
    Withdrawn,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalState {
    Pending,
    Approved,
    Rejected,
    Merged,
    Withdrawn,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleState {
    Draft,
    Active,
    Superseded,
    Disabled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleEffect {
    Allow,
    Warn,
    RequireApproval,
    Block,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Queued,
    Leased,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AiRunState {
    Started,
    Running,
    Completed,
    Failed,
    Cancelled,
    Replayed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionEnvelopeState {
    Captured,
    Sealed,
    Replayed,
    Superseded,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Observed,
    Inferred,
    Proposed,
    Approved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CrossMapRelation {
    EquivalentTo,
    SameIdentity,
    BroaderThan,
    NarrowerThan,
    RelatedTo,
    DerivedFrom,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct VersionToken {
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TenantRef {
    pub tenant_id: TenantId,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AreaRef {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct SourceSummary {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub source_id: SourceId,
    pub current_version_id: Option<SourceVersionId>,
    pub state: SourceState,
    pub title: Option<String>,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct MemorySummary {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub memory_id: MemoryId,
    pub current_version_id: Option<MemoryVersionId>,
    pub state: MemoryState,
    pub origin: Origin,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct ProposalSummary {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub proposal_id: ProposalId,
    pub state: ProposalState,
    pub origin: Origin,
    pub rejection_reason: Option<String>,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct OperationSummary {
    pub tenant_id: TenantId,
    pub operation_id: OperationId,
    pub state: OperationState,
    pub attempt: u32,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: OffsetDateTime,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: OffsetDateTime,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct EventRecord {
    pub event_id: EventId,
    pub tenant_id: TenantId,
    pub actor_id: Option<ActorId>,
    pub agent_identity_id: Option<AgentIdentityId>,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub request_id: String,
    pub idempotency_key: Option<String>,
    #[schema(value_type = String, format = DateTime)]
    pub occurred_at: OffsetDateTime,
    pub schema_version: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_uuidv7() {
        let id = TenantId::new_v7();
        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn state_serialization_is_stable() {
        assert_eq!(
            serde_json::to_string(&SourceState::PartiallyReady).unwrap(),
            "\"partially_ready\""
        );
        assert_eq!(
            serde_json::to_string(&RuleEffect::RequireApproval).unwrap(),
            "\"require_approval\""
        );
    }
}
