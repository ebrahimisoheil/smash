//! Declarative, deterministic policy evaluation for every governed boundary.
//!
//! Rules are data, never executable user code.  Adapters must call this module
//! before doing work; the envelope is useful context for a model but is not an
//! enforcement mechanism.
#![forbid(unsafe_code)]

use engrave_contracts::{
    AgentIdentityId, AreaId, RuleEffect, RuleId, RuleState, RuleVersionId, TenantId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use time::OffsetDateTime;
use uuid::Uuid;

pub const POLICY_ENVELOPE_VERSION: &str = "phase-g.v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationPoint {
    BeforeRetrieval,
    AfterRetrieval,
    BeforeDisclosure,
    BeforeProposal,
    BeforeWrite,
    BeforeTool,
    AfterTool,
    SessionEnd,
    BeforeActivation,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectType {
    Source,
    Memory,
    Chunk,
    Proposal,
    Area,
    Tool,
    Connector,
    Session,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuleScope {
    pub tenant_id: TenantId,
    pub environment: Option<String>,
    #[serde(default)]
    pub area_ids: BTreeSet<AreaId>,
    #[serde(default)]
    pub actor_ids: BTreeSet<Uuid>,
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub roles: BTreeSet<String>,
    #[serde(default)]
    pub agent_identity_ids: BTreeSet<AgentIdentityId>,
    #[serde(default)]
    pub purposes: BTreeSet<String>,
    #[serde(default)]
    pub session_ids: BTreeSet<Uuid>,
    #[serde(default)]
    pub object_types: BTreeSet<ObjectType>,
    #[serde(default)]
    pub connectors: BTreeSet<String>,
    #[serde(default)]
    pub tools: BTreeSet<String>,
}

impl Default for RuleScope {
    fn default() -> Self {
        Self {
            tenant_id: TenantId::new(Uuid::nil()),
            environment: None,
            area_ids: BTreeSet::new(),
            actor_ids: BTreeSet::new(),
            persona: None,
            roles: BTreeSet::new(),
            agent_identity_ids: BTreeSet::new(),
            purposes: BTreeSet::new(),
            session_ids: BTreeSet::new(),
            object_types: BTreeSet::new(),
            connectors: BTreeSet::new(),
            tools: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuleConditions {
    pub source_classes: BTreeSet<String>,
    pub memory_types: BTreeSet<String>,
    pub fields: BTreeSet<String>,
    pub sensitivities: BTreeSet<String>,
    pub lifecycle_states: BTreeSet<String>,
    pub actions: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub id: RuleId,
    pub version_id: RuleVersionId,
    pub version_number: u64,
    pub scope: RuleScope,
    pub conditions: RuleConditions,
    pub evaluation_points: BTreeSet<EvaluationPoint>,
    pub priority: i32,
    pub locked: bool,
    pub effect: RuleEffect,
    pub rationale: String,
    pub state: RuleState,
    pub effective_from: Option<OffsetDateTime>,
    pub effective_until: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuleRequest {
    pub tenant_id: TenantId,
    pub environment: String,
    pub actor_id: Option<Uuid>,
    pub persona: Option<String>,
    pub role: Option<String>,
    pub agent_identity_id: Option<AgentIdentityId>,
    pub area_id: Option<AreaId>,
    pub purpose: String,
    pub session_id: Option<Uuid>,
    pub point: EvaluationPoint,
    pub object_type: ObjectType,
    pub object_class: Option<String>,
    pub memory_type: Option<String>,
    pub sensitivity: Option<String>,
    pub lifecycle: Option<String>,
    pub fields: BTreeSet<String>,
    pub action: Option<String>,
    pub connector: Option<String>,
    pub tool: Option<String>,
    pub now: OffsetDateTime,
    pub permitted_area_ids: BTreeSet<AreaId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolicyEnvelope {
    pub version: String,
    pub allowed_area_ids: BTreeSet<AreaId>,
    pub allowed_object_types: BTreeSet<ObjectType>,
    pub allowed_fields: BTreeSet<String>,
    pub blocked_actions: BTreeSet<String>,
    pub approval_requirements: BTreeSet<String>,
    pub rule_ids: Vec<(RuleId, RuleVersionId)>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuleDecision {
    pub rule_id: RuleId,
    pub rule_version: RuleVersionId,
    pub effect: RuleEffect,
    pub rationale: String,
    pub next_action: String,
    pub envelope: PolicyEnvelope,
    pub conflict: bool,
    pub actor_id: Option<Uuid>,
    pub purpose: String,
    pub evaluation_point: EvaluationPoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuleEvaluationError {
    InvalidConfiguration(String),
    Conflict(Vec<(RuleId, RuleVersionId)>),
}

#[derive(Clone, Debug, Default)]
pub struct RuleEvaluator {
    rules: Vec<Rule>,
}

impl RuleEvaluator {
    pub fn new(rules: Vec<Rule>) -> Result<Self, RuleEvaluationError> {
        for rule in &rules {
            if rule.scope.tenant_id == TenantId::new(Uuid::nil())
                || rule.rationale.trim().is_empty()
            {
                return Err(RuleEvaluationError::InvalidConfiguration(
                    "tenant and rationale are required".into(),
                ));
            }
            if rule.state == RuleState::Active && rule.evaluation_points.is_empty() {
                return Err(RuleEvaluationError::InvalidConfiguration(
                    "active Rule has no evaluation point".into(),
                ));
            }
            if let (Some(start), Some(end)) = (rule.effective_from, rule.effective_until) {
                if end < start {
                    return Err(RuleEvaluationError::InvalidConfiguration(
                        "effective_until precedes effective_from".into(),
                    ));
                }
            }
        }
        Ok(Self { rules })
    }

    pub fn preflight(&self, request: &RuleRequest) -> Result<RuleDecision, RuleEvaluationError> {
        let mut matched: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|r| matches_rule(r, request))
            .collect();
        matched.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.id.cmp(&b.id))
                .then_with(|| a.version_id.cmp(&b.version_id))
        });
        let ids = matched
            .iter()
            .map(|r| (r.id, r.version_id))
            .collect::<Vec<_>>();
        let has_block = matched.iter().any(|r| r.effect == RuleEffect::Block);
        let has_allow = matched.iter().any(|r| r.effect == RuleEffect::Allow);
        if has_block && has_allow {
            return Err(RuleEvaluationError::Conflict(ids));
        }
        let winner = matched.first().copied();
        let (rule_id, rule_version, effect, rationale) = winner
            .map(|r| (r.id, r.version_id, r.effect, r.rationale.clone()))
            .unwrap_or_else(|| {
                (
                    RuleId::new(Uuid::nil()),
                    RuleVersionId::new(Uuid::nil()),
                    RuleEffect::Allow,
                    "No matching Rule; baseline tenant/Area grant applies".into(),
                )
            });
        let mut allowed_areas = request.permitted_area_ids.clone();
        let mut allowed_types = BTreeSet::from([request.object_type]);
        let mut allowed_fields = request.fields.clone();
        let mut blocked_actions = BTreeSet::new();
        let mut approval_requirements = BTreeSet::new();
        for rule in &matched {
            if !rule.scope.area_ids.is_empty() {
                allowed_areas.retain(|a| rule.scope.area_ids.contains(a));
            }
            if !rule.conditions.fields.is_empty() {
                allowed_fields.retain(|f| rule.conditions.fields.contains(f));
            }
            if rule.effect == RuleEffect::Block {
                if let Some(action) = &request.action {
                    blocked_actions.insert(action.clone());
                }
            }
            if rule.effect == RuleEffect::RequireApproval {
                approval_requirements.insert(rule.rationale.clone());
            }
            if rule.effect == RuleEffect::Block {
                allowed_types.clear();
            }
        }
        let next_action = match effect {
            RuleEffect::Allow => "proceed",
            RuleEffect::Warn => "proceed_with_warning",
            RuleEffect::RequireApproval => "obtain_explicit_approval",
            RuleEffect::Block => "do_not_execute",
        }
        .into();
        Ok(RuleDecision {
            rule_id,
            rule_version,
            effect,
            rationale,
            next_action,
            envelope: PolicyEnvelope {
                version: POLICY_ENVELOPE_VERSION.into(),
                allowed_area_ids: allowed_areas,
                allowed_object_types: allowed_types,
                allowed_fields,
                blocked_actions,
                approval_requirements,
                rule_ids: ids,
            },
            conflict: false,
            actor_id: request.actor_id,
            purpose: request.purpose.clone(),
            evaluation_point: request.point,
        })
    }
}

fn matches_rule(rule: &Rule, request: &RuleRequest) -> bool {
    rule.state == RuleState::Active
        && rule.scope.tenant_id == request.tenant_id
        && rule.evaluation_points.contains(&request.point)
        && rule.effective_from.is_none_or(|v| request.now >= v)
        && rule.effective_until.is_none_or(|v| request.now <= v)
        && (rule.scope.environment.is_none()
            || rule.scope.environment.as_ref() == Some(&request.environment))
        && (rule.scope.area_ids.is_empty()
            || request
                .area_id
                .is_some_and(|a| rule.scope.area_ids.contains(&a)))
        && (rule.scope.actor_ids.is_empty()
            || request
                .actor_id
                .is_some_and(|a| rule.scope.actor_ids.contains(&a)))
        && (rule.scope.persona.is_none() || rule.scope.persona == request.persona)
        && (rule.scope.roles.is_empty()
            || request
                .role
                .as_ref()
                .is_some_and(|r| rule.scope.roles.contains(r)))
        && (rule.scope.agent_identity_ids.is_empty()
            || request
                .agent_identity_id
                .is_some_and(|a| rule.scope.agent_identity_ids.contains(&a)))
        && (rule.scope.purposes.is_empty() || rule.scope.purposes.contains(&request.purpose))
        && (rule.scope.session_ids.is_empty()
            || request
                .session_id
                .is_some_and(|s| rule.scope.session_ids.contains(&s)))
        && (rule.scope.object_types.is_empty()
            || rule.scope.object_types.contains(&request.object_type))
        && (rule.scope.connectors.is_empty()
            || request
                .connector
                .as_ref()
                .is_some_and(|c| rule.scope.connectors.contains(c)))
        && (rule.scope.tools.is_empty()
            || request
                .tool
                .as_ref()
                .is_some_and(|t| rule.scope.tools.contains(t)))
        && (rule.conditions.source_classes.is_empty()
            || request
                .object_class
                .as_ref()
                .is_some_and(|v| rule.conditions.source_classes.contains(v)))
        && (rule.conditions.memory_types.is_empty()
            || request
                .memory_type
                .as_ref()
                .is_some_and(|v| rule.conditions.memory_types.contains(v)))
        && (rule.conditions.sensitivities.is_empty()
            || request
                .sensitivity
                .as_ref()
                .is_some_and(|v| rule.conditions.sensitivities.contains(v)))
        && (rule.conditions.lifecycle_states.is_empty()
            || request
                .lifecycle
                .as_ref()
                .is_some_and(|v| rule.conditions.lifecycle_states.contains(v)))
        && (rule.conditions.actions.is_empty()
            || request
                .action
                .as_ref()
                .is_some_and(|v| rule.conditions.actions.contains(v)))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCall {
    pub connector: String,
    pub tool: String,
    pub argument_hash: String,
}

#[derive(Clone, Debug)]
pub struct PreToolGateway {
    evaluator: RuleEvaluator,
}

impl PreToolGateway {
    pub fn new(evaluator: RuleEvaluator) -> Self {
        Self { evaluator }
    }
    pub fn check(
        &self,
        mut request: RuleRequest,
        call: &ToolCall,
    ) -> Result<RuleDecision, RuleEvaluationError> {
        request.point = EvaluationPoint::BeforeTool;
        request.object_type = ObjectType::Tool;
        request.connector = Some(call.connector.clone());
        request.tool = Some(call.tool.clone());
        self.evaluator.preflight(&request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule(effect: RuleEffect, locked: bool, priority: i32) -> Rule {
        Rule {
            id: RuleId::new(Uuid::from_u128(1 + priority as u128)),
            version_id: RuleVersionId::new(Uuid::from_u128(101 + priority as u128)),
            version_number: 1,
            scope: RuleScope {
                tenant_id: TenantId::new(Uuid::from_u128(9)),
                ..Default::default()
            },
            conditions: RuleConditions::default(),
            evaluation_points: BTreeSet::from([EvaluationPoint::BeforeRetrieval]),
            priority,
            locked,
            effect,
            rationale: "fixture policy".into(),
            state: RuleState::Active,
            effective_from: None,
            effective_until: None,
        }
    }
    fn request() -> RuleRequest {
        RuleRequest {
            tenant_id: TenantId::new(Uuid::from_u128(9)),
            environment: "test".into(),
            actor_id: None,
            persona: None,
            role: None,
            agent_identity_id: None,
            area_id: None,
            purpose: "read".into(),
            session_id: None,
            point: EvaluationPoint::BeforeRetrieval,
            object_type: ObjectType::Memory,
            object_class: None,
            memory_type: None,
            sensitivity: Some("private".into()),
            lifecycle: None,
            fields: BTreeSet::from(["claim".into()]),
            action: None,
            connector: None,
            tool: None,
            now: OffsetDateTime::UNIX_EPOCH,
            permitted_area_ids: BTreeSet::new(),
        }
    }
    #[test]
    fn locked_block_and_allow_conflict_fails_closed() {
        let e = RuleEvaluator::new(vec![
            rule(RuleEffect::Block, true, 10),
            rule(RuleEffect::Allow, false, 1),
        ])
        .unwrap();
        assert!(matches!(
            e.preflight(&request()),
            Err(RuleEvaluationError::Conflict(_))
        ));
    }
    #[test]
    fn approval_has_explicit_next_action_and_envelope() {
        let e = RuleEvaluator::new(vec![rule(RuleEffect::RequireApproval, false, 2)]).unwrap();
        let d = e.preflight(&request()).unwrap();
        assert_eq!(d.next_action, "obtain_explicit_approval");
        assert_eq!(d.envelope.version, POLICY_ENVELOPE_VERSION);
    }

    #[test]
    fn killer_path_blocks_retrieval_disclosure_and_tool_boundaries() {
        let mut blocked = rule(RuleEffect::Block, true, 50);
        blocked.evaluation_points = BTreeSet::from([
            EvaluationPoint::BeforeRetrieval,
            EvaluationPoint::BeforeDisclosure,
            EvaluationPoint::BeforeTool,
        ]);
        blocked.conditions.sensitivities.insert("private".into());
        let evaluator = RuleEvaluator::new(vec![blocked]).unwrap();

        let retrieval = evaluator.preflight(&request()).unwrap();
        assert_eq!(retrieval.effect, RuleEffect::Block);
        assert_eq!(retrieval.next_action, "do_not_execute");
        assert!(retrieval.envelope.allowed_object_types.is_empty());

        let mut disclosure = request();
        disclosure.point = EvaluationPoint::BeforeDisclosure;
        disclosure.action = Some("publish".into());
        assert_eq!(
            evaluator.preflight(&disclosure).unwrap().effect,
            RuleEffect::Block
        );

        let gateway = PreToolGateway::new(evaluator);
        let tool = gateway
            .check(
                request(),
                &ToolCall {
                    connector: "native".into(),
                    tool: "publish_source".into(),
                    argument_hash: "sha256:fixture".into(),
                },
            )
            .unwrap();
        assert_eq!(tool.effect, RuleEffect::Block);
    }
}
