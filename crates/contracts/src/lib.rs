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
}
