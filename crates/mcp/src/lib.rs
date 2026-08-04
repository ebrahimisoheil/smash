//! Transport-neutral MCP adapter. Policy remains in `engrave-core`.
#![forbid(unsafe_code)]

use async_trait::async_trait;
use engrave_contracts::{
    validate_workspace_setup_args, AgentIdentityId, AreaId, RuleEffect, TenantId,
};
use engrave_core::{
    AggressiveIntent, ApplicationError, EvaluationPoint, ObjectType, PreToolGateway, Rule,
    RuleDecision, RuleEvaluator, RuleRequest, SearchBudgets, SearchRequest, ToolCall,
};
use engrave_storage::PgRepository;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io, sync::Arc};
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SERVER_NAME: &str = "io.github.ebrahimisoheil/engrave";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const TOOL_TIMEOUT_SECONDS: u64 = 30;

#[derive(Clone, Debug, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}
#[derive(Clone, Debug, Serialize)]
pub struct RpcResponse {
    pub jsonrpc: &'static str,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}
#[derive(Clone, Debug, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RequestContext {
    pub tenant_id: TenantId,
    pub actor_id: Uuid,
    pub host_id: String,
    pub agent_identity_id: AgentIdentityId,
    pub session_id: Uuid,
    pub purpose: String,
    pub role: String,
    pub area_id: AreaId,
    #[serde(default = "default_environment")]
    pub environment: String,
}
fn default_environment() -> String {
    "local".into()
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryView {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub claim: String,
    pub evidence: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProposalView {
    pub id: Uuid,
    pub state: &'static str,
    pub area_id: AreaId,
}

/// The application/ storage adapter implements this narrow capability port.
/// It is intentionally not an authorization port: the MCP service gates first.
#[async_trait]
pub trait McpBackend: Send + Sync + 'static {
    async fn active_rules(
        &self,
        _context: &RequestContext,
    ) -> Result<Option<Vec<Rule>>, ApplicationError> {
        Ok(None)
    }
    async fn status(&self, context: &RequestContext) -> Result<Value, ApplicationError>;
    async fn recall(
        &self,
        context: &RequestContext,
        query: &str,
    ) -> Result<Vec<MemoryView>, ApplicationError>;
    async fn aggressive_search(
        &self,
        _context: &RequestContext,
        _args: &Value,
    ) -> Result<Value, ApplicationError> {
        Err(ApplicationError::DependencyUnavailable {
            dependency: "aggressive-search",
        })
    }
    async fn workspace_setup(
        &self,
        _context: &RequestContext,
        _args: &Value,
    ) -> Result<Value, ApplicationError> {
        Err(ApplicationError::DependencyUnavailable {
            dependency: "workspace-setup",
        })
    }
    async fn propose_memory(
        &self,
        context: &RequestContext,
        args: &Value,
    ) -> Result<ProposalView, ApplicationError>;
    async fn ingest_source(
        &self,
        context: &RequestContext,
        args: &Value,
    ) -> Result<Value, ApplicationError>;
    async fn review(
        &self,
        context: &RequestContext,
        args: &Value,
    ) -> Result<Value, ApplicationError>;
    async fn rules(&self, context: &RequestContext) -> Result<Value, ApplicationError>;
    async fn resource(
        &self,
        context: &RequestContext,
        uri: &str,
    ) -> Result<Value, ApplicationError>;
    async fn record_decision(
        &self,
        _context: &RequestContext,
        _decision: &RuleDecision,
        _outcome: &str,
        _argument_hash: &str,
    ) -> Result<(), ApplicationError> {
        Ok(())
    }
}

/// PostgreSQL adapter used by the real binary when `ENGRAVE_DATABASE_URL` is
/// configured. It performs identity/Area validation before every capability;
/// the MCP protocol layer remains independent of SQLx.
pub struct PgMcpBackend {
    pub repository: Arc<PgRepository>,
}

impl PgMcpBackend {
    async fn authorize(&self, c: &RequestContext) -> Result<(), ApplicationError> {
        self.repository
            .validate_mcp_context(
                c.tenant_id,
                c.actor_id,
                c.agent_identity_id,
                c.session_id,
                c.area_id,
                &c.role,
            )
            .await
    }
}

#[async_trait]
impl McpBackend for PgMcpBackend {
    async fn active_rules(
        &self,
        c: &RequestContext,
    ) -> Result<Option<Vec<Rule>>, ApplicationError> {
        self.authorize(c).await?;
        Ok(Some(self.repository.active_rules(c.tenant_id).await?))
    }
    async fn status(&self, c: &RequestContext) -> Result<Value, ApplicationError> {
        self.authorize(c).await?;
        self.repository.ping().await?;
        let rules = self.repository.active_rules(c.tenant_id).await?;
        Ok(
            json!({"server":SERVER_NAME,"version":SERVER_VERSION,"tenant_id":c.tenant_id,"area_id":c.area_id,"rules":{"active":rules.len()},"connector":{"name":"notion-source","state":"worker_backed"}}),
        )
    }
    async fn recall(
        &self,
        c: &RequestContext,
        query: &str,
    ) -> Result<Vec<MemoryView>, ApplicationError> {
        self.authorize(c).await?;
        let authorization = self
            .repository
            .resolve_search_authorization(
                c.tenant_id,
                c.actor_id,
                &[c.area_id.as_uuid()],
                c.purpose.clone(),
            )
            .await?;
        let request = SearchRequest {
            authorization,
            query: query.to_owned(),
            now: time::OffsetDateTime::now_utc(),
            token_budget: 4096,
            entry_limit: 20,
        };
        Ok(self
            .repository
            .search_lexical(&request)
            .await?
            .into_iter()
            .map(|hit| MemoryView {
                id: hit.record.memory_id.as_uuid(),
                tenant_id: hit.record.tenant_id,
                area_id: hit.record.area_id,
                claim: hit.record.claim,
                evidence: hit.record.evidence,
            })
            .collect())
    }
    async fn aggressive_search(
        &self,
        c: &RequestContext,
        args: &Value,
    ) -> Result<Value, ApplicationError> {
        self.authorize(c).await?;
        let command = args
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("start");
        let operation_id = args
            .get("operation_id")
            .and_then(Value::as_str)
            .and_then(|v| Uuid::parse_str(v).ok());
        if command == "inspect" {
            let id = operation_id.ok_or(ApplicationError::InvalidRequest {
                message: "operation_id is required".into(),
            })?;
            return Ok(
                json!({"trace": self.repository.aggressive_search_trace(c.tenant_id, id.into()).await?}),
            );
        }
        if command == "cancel" {
            let id = operation_id.ok_or(ApplicationError::InvalidRequest {
                message: "operation_id is required".into(),
            })?;
            self.repository
                .cancel_aggressive_search(c.tenant_id, id.into())
                .await?;
            return Ok(json!({"operation_id":id,"state":"cancellation_requested"}));
        }
        let explicit = args
            .get("explicit_intent")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let budgets: SearchBudgets = serde_json::from_value(args.get("budgets").cloned().unwrap_or_else(|| json!({"max_steps":8,"max_elapsed_ms":30000,"max_tokens":4000,"max_candidates":200,"max_external_calls":4}))).map_err(|_| ApplicationError::InvalidRequest { message: "invalid aggressive-search budgets".into() })?;
        let intent = AggressiveIntent {
            tenant_id: c.tenant_id,
            actor_id: c.actor_id,
            host_id: c.host_id.clone(),
            agent_identity_id: c.agent_identity_id,
            session_id: c.session_id,
            area_id: c.area_id,
            purpose: c.purpose.clone(),
            task: args
                .get("task")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            query: args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            explicit,
            connector: args
                .get("connector")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        let trace = self
            .repository
            .start_aggressive_search(
                intent,
                budgets,
                args.get("idempotency_key")
                    .and_then(Value::as_str)
                    .unwrap_or(&c.session_id.to_string()),
            )
            .await?;
        Ok(json!({"trace":trace,"state":"queued","background_worker":true}))
    }
    async fn workspace_setup(
        &self,
        c: &RequestContext,
        args: &Value,
    ) -> Result<Value, ApplicationError> {
        self.authorize(c).await?;
        self.repository
            .workspace_setup(
                c.tenant_id,
                c.actor_id,
                c.agent_identity_id,
                c.session_id,
                &c.host_id,
                &c.purpose,
                c.area_id,
                args,
            )
            .await
    }
    async fn propose_memory(
        &self,
        c: &RequestContext,
        args: &Value,
    ) -> Result<ProposalView, ApplicationError> {
        self.authorize(c).await?;
        let claim = args.get("claim").and_then(Value::as_str).ok_or_else(|| {
            ApplicationError::InvalidRequest {
                message: "claim is required".into(),
            }
        })?;
        let evidence =
            args.get("evidence")
                .cloned()
                .ok_or_else(|| ApplicationError::InvalidRequest {
                    message: "evidence is required".into(),
                })?;
        let id = Uuid::now_v7();
        self.repository.create_memory_proposal(c.tenant_id,id,c.area_id.as_uuid(),"mcp","memory",&json!({"claim":claim,"evidence":evidence,"actor_id":c.actor_id,"agent_identity_id":c.agent_identity_id,"session_id":c.session_id})).await?;
        Ok(ProposalView {
            id,
            state: "pending",
            area_id: c.area_id,
        })
    }
    async fn ingest_source(
        &self,
        c: &RequestContext,
        args: &Value,
    ) -> Result<Value, ApplicationError> {
        self.authorize(c).await?;
        let external_id = args
            .get("external_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ApplicationError::InvalidRequest {
                message: "external_id is required".into(),
            })?;
        let content = args.get("content").and_then(Value::as_str).ok_or_else(|| {
            ApplicationError::InvalidRequest {
                message: "content is required".into(),
            }
        })?;
        let idempotency = args
            .get("idempotency_key")
            .and_then(Value::as_str)
            .unwrap_or(external_id);
        let op = self
            .repository
            .queue_connector_source(
                c.tenant_id,
                c.area_id,
                "notion-source",
                external_id,
                args.get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(external_id),
                content,
                args.get("permissions").unwrap_or(&Value::Array(Vec::new())),
                idempotency,
            )
            .await?;
        Ok(
            json!({"operation_id":op,"connector":"notion-source","state":"queued","background_worker":true}),
        )
    }
    async fn review(&self, c: &RequestContext, args: &Value) -> Result<Value, ApplicationError> {
        self.authorize(c).await?;
        if !matches!(
            c.role.as_str(),
            "area_admin" | "enterprise_admin" | "ai_governance_admin"
        ) {
            return Err(ApplicationError::Forbidden);
        }
        let proposal_id = Uuid::parse_str(
            args.get("proposal_id")
                .and_then(Value::as_str)
                .ok_or_else(|| ApplicationError::InvalidRequest {
                    message: "proposal_id is required".into(),
                })?,
        )
        .map_err(|_| ApplicationError::InvalidRequest {
            message: "proposal_id must be a UUID".into(),
        })?;
        let action = args.get("action").and_then(Value::as_str).ok_or_else(|| {
            ApplicationError::InvalidRequest {
                message: "action is required".into(),
            }
        })?;
        if action == "reject" {
            self.repository
                .reject_memory_proposal(
                    c.tenant_id,
                    proposal_id,
                    c.actor_id,
                    args.get("expected_version")
                        .and_then(Value::as_i64)
                        .unwrap_or(1),
                    args.get("idempotency_key")
                        .and_then(Value::as_str)
                        .unwrap_or(&proposal_id.to_string()),
                    args.get("reason")
                        .and_then(Value::as_str)
                        .unwrap_or("rejected by reviewer"),
                )
                .await?;
            return Ok(json!({"state":"rejected","proposal_id":proposal_id}));
        }
        if action != "approve" {
            return Err(ApplicationError::InvalidRequest {
                message: "action must be approve or reject".into(),
            });
        }
        let claim = args.get("claim").and_then(Value::as_str).ok_or_else(|| {
            ApplicationError::InvalidRequest {
                message: "claim is required for approval".into(),
            }
        })?;
        let evidence = args.get("evidence").cloned().unwrap_or_else(|| json!([]));
        let memory_id = Uuid::now_v7();
        let memory_version_id = Uuid::now_v7();
        let result = self
            .repository
            .approve_memory_proposal(
                c.tenant_id,
                proposal_id,
                c.actor_id,
                args.get("expected_version")
                    .and_then(Value::as_i64)
                    .unwrap_or(1),
                args.get("idempotency_key")
                    .and_then(Value::as_str)
                    .unwrap_or(&proposal_id.to_string()),
                memory_id,
                memory_version_id,
                claim,
                args.get("scope").and_then(Value::as_str).unwrap_or("area"),
                args.get("applies_when")
                    .and_then(Value::as_str)
                    .unwrap_or("always"),
                args.get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("reviewed"),
                &evidence,
            )
            .await?;
        Ok(json!({"state":"approved","memory_id":result}))
    }
    async fn rules(&self, c: &RequestContext) -> Result<Value, ApplicationError> {
        self.authorize(c).await?;
        Ok(
            json!({"policy_envelope_version":engrave_core::POLICY_ENVELOPE_VERSION,"rules":self.repository.active_rules(c.tenant_id).await?}),
        )
    }
    async fn resource(&self, c: &RequestContext, uri: &str) -> Result<Value, ApplicationError> {
        self.authorize(c).await?;
        Ok(json!({"uri":uri,"tenant_id":c.tenant_id,"area_id":c.area_id,"evidence":[]}))
    }
    async fn record_decision(
        &self,
        c: &RequestContext,
        d: &RuleDecision,
        outcome: &str,
        hash: &str,
    ) -> Result<(), ApplicationError> {
        self.repository
            .record_mcp_decision(
                c.tenant_id,
                c.actor_id,
                &c.host_id,
                c.agent_identity_id,
                c.session_id,
                c.area_id,
                d,
                outcome,
                hash,
            )
            .await
    }
}

#[derive(Clone)]
pub struct McpService<B> {
    backend: Arc<B>,
    fallback_rules: Vec<Rule>,
}

impl<B: McpBackend> McpService<B> {
    pub fn new(backend: Arc<B>, rules: Vec<Rule>) -> Result<Self, String> {
        RuleEvaluator::new(rules.clone()).map_err(|e| format!("invalid active Rules: {e:?}"))?;
        Ok(Self {
            backend,
            fallback_rules: rules,
        })
    }
    pub async fn handle_json(&self, line: &str) -> String {
        let response = match serde_json::from_str::<RpcRequest>(line) {
            Ok(r) => self.handle(r).await,
            Err(_) => error(None, -32700, "parse error", None),
        };
        serde_json::to_string(&response).unwrap_or_else(|_| "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32603,\"message\":\"serialization failure\"}}".into())
    }
    pub async fn handle(&self, request: RpcRequest) -> RpcResponse {
        if request.jsonrpc != "2.0" {
            return error(request.id, -32600, "invalid JSON-RPC request", None);
        }
        let id = request.id.clone();
        let result = match request.method.as_str() {
            "initialize" => Ok(
                json!({"protocolVersion":PROTOCOL_VERSION,"capabilities":{"tools":{"listChanged":false},"resources":{"subscribe":false,"listChanged":false},"prompts":{"listChanged":false}},"serverInfo":{"name":SERVER_NAME,"version":SERVER_VERSION}}),
            ),
            "notifications/initialized" => Ok(Value::Null),
            "notifications/cancelled" => Ok(Value::Null),
            "tools/list" => Ok(json!({"tools":tool_descriptors()})),
            "resources/list" => Ok(
                json!({"resources":[{"uri":"engrave://context/current","name":"governed-context","description":"Governed context and evidence","mimeType":"application/json"}]}),
            ),
            "prompts/list" => Ok(json!({"prompts":prompt_descriptors()})),
            "prompts/get" => prompt_get(&request.params),
            "resources/read" => self.call_resource(&request.params).await,
            "tools/call" => self.call_tool(&request.params).await,
            _ => Err(McpError::new(-32601, "method not found", None)),
        };
        match result {
            Ok(value) => RpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(value),
                error: None,
            },
            Err(e) => error(id, e.code, &e.message, e.data),
        }
    }
    fn context(params: &Value) -> Result<RequestContext, McpError> {
        let value = params
            .get("context")
            .cloned()
            .ok_or_else(|| McpError::invalid("context is required"))?;
        serde_json::from_value(value)
            .map_err(|_| McpError::new(-32001, "invalid or incomplete session context", None))
    }
    async fn gate(
        &self,
        c: &RequestContext,
        tool: &str,
        object: ObjectType,
        action: &str,
        args: &Value,
    ) -> Result<RuleDecision, McpError> {
        let fields = BTreeSet::from(["claim".into(), "evidence".into()]);
        let connector = if tool == "ingest_source" {
            "notion-source"
        } else {
            "engrave"
        };
        let request = RuleRequest {
            tenant_id: c.tenant_id,
            environment: c.environment.clone(),
            actor_id: Some(c.actor_id),
            persona: None,
            role: Some(c.role.clone()),
            agent_identity_id: Some(c.agent_identity_id),
            area_id: Some(c.area_id),
            purpose: c.purpose.clone(),
            session_id: Some(c.session_id),
            point: EvaluationPoint::BeforeTool,
            object_type: object,
            object_class: None,
            memory_type: None,
            sensitivity: None,
            lifecycle: None,
            fields,
            action: Some(action.into()),
            connector: Some(connector.into()),
            tool: Some(tool.into()),
            now: time::OffsetDateTime::now_utc(),
            permitted_area_ids: BTreeSet::from([c.area_id]),
        };
        let rules = tokio::time::timeout(
            std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
            self.backend.active_rules(c),
        )
        .await
        .map_err(|_| timeout_error())?
        .map_err(map_application_error)?
        .unwrap_or_else(|| self.fallback_rules.clone());
        let evaluator = RuleEvaluator::new(rules).map_err(|e| {
            McpError::new(
                -32003,
                "policy evaluation failed closed",
                Some(json!({"reason":format!("{e:?}")})),
            )
        })?;
        let decision = PreToolGateway::new(evaluator)
            .check(
                request,
                &ToolCall {
                    connector: connector.into(),
                    tool: tool.into(),
                    argument_hash: argument_hash(args),
                },
            )
            .map_err(|e| {
                McpError::new(
                    -32003,
                    "policy evaluation failed closed",
                    Some(json!({"reason":format!("{e:?}")})),
                )
            })?;
        tokio::time::timeout(
            std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
            self.backend
                .record_decision(c, &decision, "evaluated", &argument_hash(args)),
        )
        .await
        .map_err(|_| timeout_error())?
        .map_err(map_application_error)?;
        match decision.effect {
            RuleEffect::Block => Err(McpError::new(
                -32003,
                "policy blocked",
                Some(decision_json(&decision)),
            )),
            RuleEffect::RequireApproval => Err(McpError::new(
                -32004,
                "explicit approval required",
                Some(decision_json(&decision)),
            )),
            _ => Ok(decision),
        }
    }
    async fn call_tool(&self, params: &Value) -> Result<Value, McpError> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| McpError::invalid("tool name is required"))?;
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let c = Self::context(&args)?;
        if name == "workspace_setup" {
            validate_workspace_setup_args(&args)
                .map_err(|message| McpError::new(-32602, message, None))?;
        }
        let object = match name {
            "recall" => ObjectType::Memory,
            "propose_memory" => ObjectType::Proposal,
            "ingest_source" => ObjectType::Source,
            "review" => ObjectType::Proposal,
            "workspace_setup" => ObjectType::Tool,
            "rules" => ObjectType::Tool,
            "status" => ObjectType::Session,
            _ => return Err(McpError::new(-32602, "unknown tool", None)),
        };
        let decision = self.gate(&c, name, object, name, &args).await?;
        let output = match name {
            "status" => tokio::time::timeout(
                std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
                self.backend.status(&c),
            )
            .await
            .map_err(|_| timeout_error())?
            .map_err(map_application_error),
            "recall" => {
                if args.get("mode").and_then(Value::as_str) == Some("aggressive") {
                    if !decision.envelope.allowed_area_ids.contains(&c.area_id) {
                        return Err(McpError::new(
                            -32003,
                            "aggressive search Area narrowed by policy",
                            Some(decision_json(&decision)),
                        ));
                    }
                    let value = tokio::time::timeout(
                        std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
                        self.backend.aggressive_search(&c, &args),
                    )
                    .await
                    .map_err(|_| timeout_error())?
                    .map_err(map_application_error)?;
                    return Ok(value);
                }
                let items = tokio::time::timeout(
                    std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
                    self.backend.recall(
                        &c,
                        args.get("query")
                            .and_then(Value::as_str)
                            .ok_or_else(|| McpError::invalid("query is required"))?,
                    ),
                )
                .await
                .map_err(|_| timeout_error())?
                .map_err(map_application_error)?;
                let disclosure = self
                    .disclosure_gate(&c, "recall", ObjectType::Memory, "disclose", &args)
                    .await?;
                Ok::<Value, McpError>(json!({"items":items,"policy_envelope":disclosure.envelope}))
            }
            "propose_memory" => tokio::time::timeout(
                std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
                self.backend.propose_memory(&c, &args),
            )
            .await
            .map_err(|_| timeout_error())?
            .map(|p| json!({"proposal":p,"policy_envelope":decision.envelope}))
            .map_err(map_application_error),
            "ingest_source" => tokio::time::timeout(
                std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
                self.backend.ingest_source(&c, &args),
            )
            .await
            .map_err(|_| timeout_error())?
            .map_err(map_application_error),
            "review" => tokio::time::timeout(
                std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
                self.backend.review(&c, &args),
            )
            .await
            .map_err(|_| timeout_error())?
            .map_err(map_application_error),
            "workspace_setup" => tokio::time::timeout(
                std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
                self.backend.workspace_setup(&c, &args),
            )
            .await
            .map_err(|_| timeout_error())?
            .map_err(map_application_error),
            "rules" => tokio::time::timeout(
                std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
                self.backend.rules(&c),
            )
            .await
            .map_err(|_| timeout_error())?
            .map_err(map_application_error),
            _ => unreachable!(),
        }?;
        if name == "workspace_setup" {
            let disclosure = self
                .disclosure_gate(&c, "workspace_setup", ObjectType::Tool, "disclose", &args)
                .await?;
            return Ok(workspace_setup_response(output, disclosure));
        }
        Ok(output)
    }
    async fn disclosure_gate(
        &self,
        c: &RequestContext,
        tool: &str,
        object: ObjectType,
        action: &str,
        args: &Value,
    ) -> Result<RuleDecision, McpError> {
        let rules = tokio::time::timeout(
            std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
            self.backend.active_rules(c),
        )
        .await
        .map_err(|_| timeout_error())?
        .map_err(map_application_error)?
        .unwrap_or_else(|| self.fallback_rules.clone());
        let evaluator = RuleEvaluator::new(rules).map_err(|e| {
            McpError::new(
                -32003,
                "policy evaluation failed closed",
                Some(json!({"reason":format!("{e:?}")})),
            )
        })?;
        let decision = evaluator
            .preflight(&RuleRequest {
                tenant_id: c.tenant_id,
                environment: c.environment.clone(),
                actor_id: Some(c.actor_id),
                persona: None,
                role: Some(c.role.clone()),
                agent_identity_id: Some(c.agent_identity_id),
                area_id: Some(c.area_id),
                purpose: c.purpose.clone(),
                session_id: Some(c.session_id),
                point: EvaluationPoint::BeforeDisclosure,
                object_type: object,
                object_class: None,
                memory_type: None,
                sensitivity: None,
                lifecycle: None,
                fields: BTreeSet::from(["claim".into(), "evidence".into()]),
                action: Some(action.into()),
                connector: Some("engrave".into()),
                tool: Some(tool.into()),
                now: time::OffsetDateTime::now_utc(),
                permitted_area_ids: BTreeSet::from([c.area_id]),
            })
            .map_err(|e| {
                McpError::new(
                    -32003,
                    "disclosure policy failed closed",
                    Some(json!({"reason":format!("{e:?}")})),
                )
            })?;
        tokio::time::timeout(
            std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
            self.backend.record_decision(
                c,
                &decision,
                "disclosure_evaluated",
                &argument_hash(args),
            ),
        )
        .await
        .map_err(|_| timeout_error())?
        .map_err(map_application_error)?;
        match decision.effect {
            RuleEffect::Block => Err(McpError::new(
                -32003,
                "disclosure blocked",
                Some(decision_json(&decision)),
            )),
            RuleEffect::RequireApproval => Err(McpError::new(
                -32004,
                "disclosure approval required",
                Some(decision_json(&decision)),
            )),
            _ => Ok(decision),
        }
    }
    async fn call_resource(&self, params: &Value) -> Result<Value, McpError> {
        let c = Self::context(params)?;
        let uri = params
            .get("uri")
            .and_then(Value::as_str)
            .ok_or_else(|| McpError::invalid("uri is required"))?;
        let d = self
            .gate(&c, "resources/read", ObjectType::Chunk, "read", params)
            .await?;
        let value = tokio::time::timeout(
            std::time::Duration::from_secs(TOOL_TIMEOUT_SECONDS),
            self.backend.resource(&c, uri),
        )
        .await
        .map_err(|_| timeout_error())?
        .map_err(map_application_error)?;
        Ok(
            json!({"contents":[{"uri":uri,"mimeType":"application/json","text":serde_json::to_string(&json!({"data":value,"policy_envelope":d.envelope})).unwrap_or_default()}]}),
        )
    }
}

#[derive(Debug)]
struct McpError {
    code: i32,
    message: String,
    data: Option<Value>,
}
impl McpError {
    fn new(code: i32, message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            code,
            message: message.into(),
            data,
        }
    }
    fn invalid(message: impl Into<String>) -> Self {
        Self::new(-32602, message, None)
    }
}
fn error(id: Option<Value>, code: i32, message: &str, data: Option<Value>) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(RpcError {
            code,
            message: message.into(),
            data,
        }),
    }
}
fn decision_json(d: &RuleDecision) -> Value {
    json!({"rule_id":d.rule_id,"rule_version":d.rule_version,"rationale":d.rationale,"next_action":d.next_action,"policy_envelope":d.envelope})
}

fn workspace_setup_response(mut value: Value, disclosure: RuleDecision) -> Value {
    let markdown = format_workspace_markdown(&value);
    let structured = value.clone();
    if let Value::Object(ref mut object) = value {
        object.insert("structuredContent".into(), structured);
        object.insert("content".into(), json!([{"type":"text","text":markdown}]));
        object.insert("policy_envelope".into(), json!(disclosure.envelope));
    }
    value
}

fn format_workspace_markdown(value: &Value) -> String {
    let state = value
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let interview = value
        .get("interview_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let proposal = value
        .get("proposal_id")
        .and_then(Value::as_str)
        .map(|id| format!("\nProposal: `{id}` (pending review only)"))
        .unwrap_or_default();
    format!(
        "# Workspace setup\n\nState: `{state}`\nInterview: `{interview}`{proposal}\n\nNothing is activated: Area grants, Map publication, and ontology admission remain separately governed."
    )
}
fn map_application_error(e: ApplicationError) -> McpError {
    McpError::new(
        -32000,
        e.code().as_str(),
        Some(json!({"error_code":e.code().as_str(),"message":e.to_string()})),
    )
}
fn timeout_error() -> McpError {
    McpError::new(
        -32005,
        "operation timed out",
        Some(json!({"retryable":true})),
    )
}

/// Hashes the argument shape while excluding credential material.
pub fn argument_hash(args: &Value) -> String {
    fn scrub(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .filter(|(k, _)| {
                        !matches!(
                            k.as_str(),
                            "token"
                                | "secret"
                                | "password"
                                | "api_key"
                                | "authorization"
                                | "credential"
                                | "access_token"
                                | "refresh_token"
                        )
                    })
                    .map(|(k, v)| (k.clone(), scrub(v)))
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(scrub).collect()),
            _ => v.clone(),
        }
    }
    format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&scrub(args)).unwrap_or_default())
    )
}
fn tool_descriptors() -> Vec<Value> {
    vec![
        json!({"name":"status","description":"Read session, tenant, Area, Rule, and connector health.","inputSchema":{"type":"object","required":["context"],"properties":{"context":{"type":"object"}}}}),
        json!({"name":"recall","description":"Retrieve governed reviewed Memory for the active Area and purpose.","inputSchema":{"type":"object","required":["context","query"],"properties":{"context":{"type":"object"},"query":{"type":"string"}}}}),
        json!({"name":"propose_memory","description":"Create a Memory Proposal; never activates durable Memory.","inputSchema":{"type":"object","required":["context","claim","evidence"],"properties":{"context":{"type":"object"},"claim":{"type":"string"},"evidence":{"type":"array","items":{"type":"string"}}}}}),
        json!({"name":"ingest_source","description":"Queue governed Source ingestion for the background worker.","inputSchema":{"type":"object","required":["context","external_id","content"],"properties":{"context":{"type":"object"},"external_id":{"type":"string"},"content":{"type":"string"}}}}),
        json!({"name":"review","description":"Review an eligible Proposal with explicit reviewer context.","inputSchema":{"type":"object","required":["context","proposal_id","action"],"properties":{"context":{"type":"object"},"proposal_id":{"type":"string"},"action":{"type":"string","enum":["approve","reject"]}}}}),
        json!({"name":"workspace_setup","description":"Conduct a governed workspace and ontology interview; submission creates only a Proposal.","inputSchema":{"type":"object","required":["context","action"],"properties":{"context":{"type":"object"},"action":{"type":"string","enum":["begin","draft","confirm","submit","inspect","cancel"]},"interview_id":{"type":"string"},"selected_area_ids":{"type":"array","items":{"type":"string"}},"requested_areas":{"type":"array"},"ontology_draft":{"type":"object"},"confirmed":{"type":"boolean"},"idempotency_key":{"type":"string"}}}}),
        json!({"name":"rules","description":"Inspect effective Rule metadata and envelope without secrets or hidden content.","inputSchema":{"type":"object","required":["context"],"properties":{"context":{"type":"object"}}}}),
    ]
}
fn prompt_descriptors() -> Vec<Value> {
    vec![
        json!({"name":"start-with-engrave","description":"Start a governed session with status and scoped recall.","arguments":[]}),
        json!({"name":"close-session","description":"Capture observations as Proposals only.","arguments":[]}),
    ]
}
fn prompt_get(p: &Value) -> Result<Value, McpError> {
    match p.get("name").and_then(Value::as_str) {
        Some("start-with-engrave") => Ok(
            json!({"description":"Version 1.0.0 session-start workflow","messages":[{"role":"user","content":{"type":"text","text":"Use status, then recall only for the active Area and stated purpose. The server enforces access."}}]}),
        ),
        Some("close-session") => Ok(
            json!({"description":"Version 1.0.0 session-end workflow","messages":[{"role":"user","content":{"type":"text","text":"Summarize candidate observations and call propose_memory. Do not activate Memory silently."}}]}),
        ),
        _ => Err(McpError::new(-32602, "unknown prompt", None)),
    }
}

#[derive(Default)]
pub struct InMemoryBackend {
    memories: std::sync::RwLock<Vec<MemoryView>>,
    proposals: std::sync::RwLock<Vec<ProposalView>>,
    decisions: std::sync::RwLock<Vec<String>>,
    rules: std::sync::RwLock<Vec<Rule>>,
    workspace_interviews: std::sync::RwLock<std::collections::BTreeMap<Uuid, Value>>,
}
impl InMemoryBackend {
    pub fn with_memory(self, m: MemoryView) -> Self {
        self.memories.write().unwrap().push(m);
        self
    }
    pub fn with_rules(self, rules: Vec<Rule>) -> Self {
        *self.rules.write().unwrap() = rules;
        self
    }
}
#[async_trait]
impl McpBackend for InMemoryBackend {
    async fn active_rules(
        &self,
        _: &RequestContext,
    ) -> Result<Option<Vec<Rule>>, ApplicationError> {
        let rules = self.rules.read().unwrap().clone();
        Ok((!rules.is_empty()).then_some(rules))
    }
    async fn status(&self, c: &RequestContext) -> Result<Value, ApplicationError> {
        Ok(
            json!({"server":SERVER_NAME,"version":SERVER_VERSION,"tenant_id":c.tenant_id,"area_id":c.area_id,"connector":{"name":"notion-source","state":"configured","sync":"worker_only"}}),
        )
    }
    async fn recall(
        &self,
        c: &RequestContext,
        q: &str,
    ) -> Result<Vec<MemoryView>, ApplicationError> {
        Ok(self
            .memories
            .read()
            .unwrap()
            .iter()
            .filter(|m| {
                m.tenant_id == c.tenant_id
                    && m.area_id == c.area_id
                    && m.claim.to_lowercase().contains(&q.to_lowercase())
            })
            .cloned()
            .collect())
    }
    async fn aggressive_search(
        &self,
        _: &RequestContext,
        args: &Value,
    ) -> Result<Value, ApplicationError> {
        if args.get("explicit_intent").and_then(Value::as_bool) != Some(true) {
            return Err(ApplicationError::InvalidRequest {
                message: "explicit_intent must be true".into(),
            });
        }
        Ok(json!({"state":"queued","mode":"aggressive","background_worker":true}))
    }
    async fn workspace_setup(
        &self,
        c: &RequestContext,
        args: &Value,
    ) -> Result<Value, ApplicationError> {
        let action = args.get("action").and_then(Value::as_str).ok_or_else(|| {
            ApplicationError::InvalidRequest {
                message: "workspace_setup action is required".into(),
            }
        })?;
        let selected = args
            .get("selected_area_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_else(|| vec![json!(c.area_id.as_uuid())]);
        if selected.iter().any(|area| {
            area.as_str()
                .and_then(|value| Uuid::parse_str(value).ok())
                .is_none_or(|area| area != c.area_id.as_uuid())
        }) {
            return Err(ApplicationError::Forbidden);
        }
        let key = args
            .get("idempotency_key")
            .and_then(Value::as_str)
            .unwrap_or("workspace-setup");
        if action == "begin" {
            if let Some(existing) = self
                .workspace_interviews
                .read()
                .unwrap()
                .values()
                .find(|value| {
                    value.get("tenant_id") == Some(&json!(c.tenant_id))
                        && value.get("actor_id") == Some(&json!(c.actor_id))
                        && value.get("agent_identity_id") == Some(&json!(c.agent_identity_id))
                        && value.get("session_id") == Some(&json!(c.session_id))
                        && value.get("idempotency_key").and_then(Value::as_str) == Some(key)
                })
                .cloned()
            {
                return Ok(existing);
            }
            let interview_id = Uuid::now_v7();
            let value = json!({
                "interview_id": interview_id,
                "tenant_id": c.tenant_id,
                "actor_id": c.actor_id,
                "agent_identity_id": c.agent_identity_id,
                "session_id": c.session_id,
                "state": "collecting",
                "selected_area_ids": selected,
                "authorized_area_ids": [c.area_id],
                "request_new_area": true,
                "requested_areas": args.get("requested_areas").cloned().unwrap_or_else(|| json!([])),
                "ontology_draft": args.get("ontology_draft").cloned().unwrap_or_else(|| json!({})),
                "confirmed": false,
                "idempotency_key": key,
                "version": 1,
                "content": format!("# Workspace setup\nState: collecting\nInterview: {interview_id}\nSubmission creates a proposal only; Area access and Map publication remain separately governed.")
            });
            self.workspace_interviews
                .write()
                .unwrap()
                .insert(interview_id, value.clone());
            return Ok(value);
        }
        let interview_id = Uuid::parse_str(
            args.get("interview_id")
                .and_then(Value::as_str)
                .ok_or_else(|| ApplicationError::InvalidRequest {
                    message: "interview_id is required after begin".into(),
                })?,
        )
        .map_err(|_| ApplicationError::InvalidRequest {
            message: "interview_id must be a UUID".into(),
        })?;
        let mut interviews = self.workspace_interviews.write().unwrap();
        let interview = interviews
            .get_mut(&interview_id)
            .ok_or(ApplicationError::Forbidden)?;
        if interview.get("tenant_id") != Some(&json!(c.tenant_id))
            || interview.get("actor_id") != Some(&json!(c.actor_id))
            || interview.get("agent_identity_id") != Some(&json!(c.agent_identity_id))
            || interview.get("session_id") != Some(&json!(c.session_id))
        {
            return Err(ApplicationError::Forbidden);
        }
        let state = interview
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match action {
            "draft" if matches!(state, "collecting" | "awaiting_confirmation") => {
                interview["state"] = json!("awaiting_confirmation");
                interview["ontology_draft"] = args
                    .get("ontology_draft")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                interview["version"] = json!(interview["version"].as_i64().unwrap_or(1) + 1);
            }
            "confirm" if state == "confirmed" && interview["confirmed"] == json!(true) => {}
            "confirm"
                if state == "awaiting_confirmation"
                    && args.get("confirmed").and_then(Value::as_bool) == Some(true) =>
            {
                interview["state"] = json!("confirmed");
                interview["confirmed"] = json!(true);
                interview["version"] = json!(interview["version"].as_i64().unwrap_or(1) + 1);
            }
            "submit" if state == "submitted" => {}
            "submit" if state == "confirmed" && interview["confirmed"] == json!(true) => {
                let proposal_id = Uuid::now_v7();
                interview["state"] = json!("submitted");
                interview["proposal_id"] = json!(proposal_id);
                interview["version"] = json!(interview["version"].as_i64().unwrap_or(1) + 1);
            }
            "inspect" => {}
            "cancel" if state == "cancelled" => {}
            "cancel" if state != "submitted" => {
                interview["state"] = json!("cancelled");
                interview["version"] = json!(interview["version"].as_i64().unwrap_or(1) + 1);
            }
            _ => {
                return Err(ApplicationError::InvalidRequest {
                    message: "invalid workspace interview state transition".into(),
                })
            }
        }
        Ok(interview.clone())
    }
    async fn propose_memory(
        &self,
        c: &RequestContext,
        _: &Value,
    ) -> Result<ProposalView, ApplicationError> {
        let p = ProposalView {
            id: Uuid::now_v7(),
            state: "pending",
            area_id: c.area_id,
        };
        self.proposals.write().unwrap().push(p.clone());
        Ok(p)
    }
    async fn ingest_source(
        &self,
        c: &RequestContext,
        a: &Value,
    ) -> Result<Value, ApplicationError> {
        Ok(
            json!({"operation":"queued","tenant_id":c.tenant_id,"area_id":c.area_id,"external_id":a.get("external_id"),"background_worker":true}),
        )
    }
    async fn review(&self, _: &RequestContext, _: &Value) -> Result<Value, ApplicationError> {
        Ok(json!({"state":"reviewed"}))
    }
    async fn rules(&self, _: &RequestContext) -> Result<Value, ApplicationError> {
        Ok(json!({"policy_envelope_version":engrave_core::POLICY_ENVELOPE_VERSION,"rules":[]}))
    }
    async fn resource(&self, c: &RequestContext, uri: &str) -> Result<Value, ApplicationError> {
        Ok(json!({"uri":uri,"tenant_id":c.tenant_id,"area_id":c.area_id,"evidence":[]}))
    }
    async fn record_decision(
        &self,
        _: &RequestContext,
        _: &RuleDecision,
        outcome: &str,
        hash: &str,
    ) -> Result<(), ApplicationError> {
        self.decisions
            .write()
            .unwrap()
            .push(format!("{outcome}:{hash}"));
        Ok(())
    }
}

pub async fn run_stdio<B: McpBackend>(service: McpService<B>) -> io::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    let mut output = tokio::io::stdout();
    while let Some(line) = input.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        output
            .write_all(service.handle_json(&line).await.as_bytes())
            .await?;
        output.write_all(b"\n").await?;
        output.flush().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c() -> Value {
        json!({"tenant_id":Uuid::from_u128(1),"actor_id":Uuid::from_u128(2),"host_id":"host-test","agent_identity_id":Uuid::from_u128(3),"session_id":Uuid::from_u128(4),"purpose":"read","role":"normal_user","area_id":Uuid::from_u128(5)})
    }
    fn s() -> McpService<InMemoryBackend> {
        McpService::new(
            Arc::new(InMemoryBackend::default().with_memory(MemoryView {
                id: Uuid::from_u128(6),
                tenant_id: TenantId::new(Uuid::from_u128(1)),
                area_id: AreaId::new(Uuid::from_u128(5)),
                claim: "UTC is preferred".into(),
                evidence: vec!["source-v1#chunk-1".into()],
            })),
            vec![],
        )
        .unwrap()
    }
    #[tokio::test]
    async fn protocol_surface() {
        let s = s();
        assert!(s
            .handle_json(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
            .await
            .contains("propose_memory"));
        assert!(s
            .handle_json(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#)
            .await
            .contains(PROTOCOL_VERSION));
        let names: Vec<String> = tool_descriptors()
            .iter()
            .filter_map(|v| v.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect();
        assert_eq!(
            names,
            vec![
                "status",
                "recall",
                "propose_memory",
                "ingest_source",
                "review",
                "workspace_setup",
                "rules"
            ]
        );
    }

    #[tokio::test]
    async fn workspace_setup_is_explicit_proposal_only_and_replay_safe() {
        let service = s();
        let begin = service
            .handle_json(
                &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":c(),"action":"begin","idempotency_key":"interview-1"}}}).to_string(),
            )
            .await;
        let begin_value: Value = serde_json::from_str(&begin).unwrap();
        let interview_id = begin_value["result"]["interview_id"].as_str().unwrap();
        assert!(begin_value["result"]["structuredContent"]["interview_id"].is_string());
        assert_eq!(begin_value["result"]["content"][0]["type"], json!("text"));
        assert!(begin_value["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Nothing is activated"));
        let draft = service
            .handle_json(
                &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":c(),"action":"draft","interview_id":interview_id,"ontology_draft":{"kinds":["customer"],"relationships":[],"assumptions":[],"unresolved_questions":[]}}}}).to_string(),
            )
            .await;
        assert!(draft.contains("awaiting_confirmation"), "{draft}");
        let confirmed = service
            .handle_json(
                &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":c(),"action":"confirm","interview_id":interview_id,"confirmed":true}}}).to_string(),
            )
            .await;
        assert!(confirmed.contains("confirmed"), "{confirmed}");
        let submitted = service
            .handle_json(
                &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":c(),"action":"submit","interview_id":interview_id}}}).to_string(),
            )
            .await;
        assert!(submitted.contains("submitted"), "{submitted}");
        assert!(submitted.contains("proposal_id"), "{submitted}");
        let denied = service
            .handle_json(
                &json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":c(),"action":"begin","selected_area_ids":[Uuid::from_u128(99).to_string()]}}}).to_string(),
            )
            .await;
        assert!(
            denied.contains("-32000") || denied.contains("Forbidden"),
            "{denied}"
        );
    }

    #[tokio::test]
    async fn aggressive_recall_requires_explicit_intent_and_keeps_single_tool_surface() {
        let service = s();
        let denied = service.handle_json(&serde_json::to_string(&json!({
            "jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"recall","arguments":{"context":c(),"mode":"aggressive","explicit_intent":false,"query":"verify"}}
        })).unwrap()).await;
        assert!(denied.contains("explicit_intent must be true"));
        let accepted = service.handle_json(&serde_json::to_string(&json!({
            "jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"recall","arguments":{"context":c(),"mode":"aggressive","explicit_intent":true,"query":"verify","task":"verify evidence"}}
        })).unwrap()).await;
        assert!(accepted.contains("background_worker"));
        assert!(!accepted.contains("aggressive_search"));
    }
    #[tokio::test]
    async fn two_hosts_same_memory() {
        let s = s();
        for agent in [3, 7] {
            let mut x = c();
            x["agent_identity_id"] = json!(Uuid::from_u128(agent));
            let r = json!({"jsonrpc":"2.0","id":agent,"method":"tools/call","params":{"name":"recall","arguments":{"context":x,"query":"UTC"}}});
            assert!(s
                .handle_json(&r.to_string())
                .await
                .contains("UTC is preferred"));
        }
    }
    #[tokio::test]
    async fn proposal_pending_and_hash_secret_free() {
        let s = s();
        let r = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"propose_memory","arguments":{"context":c(),"claim":"x","evidence":["source-v1#chunk-1"],"access_token":"never-log-me"}}});
        assert!(s.handle_json(&r.to_string()).await.contains("pending"));
        assert_eq!(
            argument_hash(&json!({"x":1,"access_token":"a"})),
            argument_hash(&json!({"x":1,"access_token":"b"}))
        );
    }
    #[tokio::test]
    async fn malformed() {
        let s = s();
        assert!(s.handle_json("not json").await.contains("-32700"));
        assert!(s
            .handle_json(r#"{"jsonrpc":"2.0","id":1,"method":"nope"}"#)
            .await
            .contains("-32601"));
    }

    #[tokio::test]
    async fn protocol_resources_prompts_and_cancellation_are_stable() {
        let s = s();
        for method in ["resources/list", "prompts/list", "notifications/cancelled"] {
            let response = s
                .handle_json(&json!({"jsonrpc":"2.0","id":2,"method":method}).to_string())
                .await;
            assert!(!response.contains("-32601"), "{method} was not recognized");
        }
        let prompt = s.handle_json(r#"{"jsonrpc":"2.0","id":3,"method":"prompts/get","params":{"name":"close-session"}}"#).await;
        assert!(prompt.contains("propose_memory"));
    }

    #[tokio::test]
    async fn missing_context_is_rejected_before_backend_execution() {
        let s = s();
        let response = s.handle_json(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"status","arguments":{}}}"#).await;
        assert!(response.contains("context is required"));
    }

    #[tokio::test]
    async fn active_rule_is_reloaded_and_block_never_reaches_backend() {
        let rule = Rule {
            id: engrave_contracts::RuleId::new(Uuid::from_u128(20)),
            version_id: engrave_contracts::RuleVersionId::new(Uuid::from_u128(21)),
            version_number: 2,
            scope: engrave_core::RuleScope {
                tenant_id: TenantId::new(Uuid::from_u128(1)),
                tools: BTreeSet::from(["recall".into()]),
                ..Default::default()
            },
            conditions: engrave_core::RuleConditions::default(),
            evaluation_points: BTreeSet::from([EvaluationPoint::BeforeTool]),
            priority: 100,
            locked: true,
            effect: RuleEffect::Block,
            rationale: "private evidence is blocked".into(),
            state: engrave_contracts::RuleState::Active,
            effective_from: None,
            effective_until: None,
        };
        let backend = Arc::new(
            InMemoryBackend::default()
                .with_memory(MemoryView {
                    id: Uuid::from_u128(6),
                    tenant_id: TenantId::new(Uuid::from_u128(1)),
                    area_id: AreaId::new(Uuid::from_u128(5)),
                    claim: "should never execute".into(),
                    evidence: vec![],
                })
                .with_rules(vec![rule]),
        );
        let s = McpService::new(backend, vec![]).unwrap();
        let response = s.handle_json(&json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"recall","arguments":{"context":c(),"query":"never"}}}).to_string()).await;
        assert!(response.contains("policy blocked"));
        assert!(!response.contains("should never execute"));
    }

    #[tokio::test]
    async fn post_retrieval_disclosure_rule_blocks_result() {
        let allow = Rule {
            id: engrave_contracts::RuleId::new(Uuid::from_u128(30)),
            version_id: engrave_contracts::RuleVersionId::new(Uuid::from_u128(31)),
            version_number: 1,
            scope: engrave_core::RuleScope {
                tenant_id: TenantId::new(Uuid::from_u128(1)),
                tools: BTreeSet::from(["recall".into()]),
                ..Default::default()
            },
            conditions: engrave_core::RuleConditions::default(),
            evaluation_points: BTreeSet::from([EvaluationPoint::BeforeTool]),
            priority: 1,
            locked: false,
            effect: RuleEffect::Allow,
            rationale: "retrieval allowed for test".into(),
            state: engrave_contracts::RuleState::Active,
            effective_from: None,
            effective_until: None,
        };
        let mut block = allow.clone();
        block.id = engrave_contracts::RuleId::new(Uuid::from_u128(32));
        block.version_id = engrave_contracts::RuleVersionId::new(Uuid::from_u128(33));
        block.evaluation_points = BTreeSet::from([EvaluationPoint::BeforeDisclosure]);
        block.conditions.actions = BTreeSet::from(["disclose".into()]);
        block.priority = 50;
        block.effect = RuleEffect::Block;
        block.rationale = "disclosure blocked for test".into();
        let backend = Arc::new(
            InMemoryBackend::default()
                .with_memory(MemoryView {
                    id: Uuid::from_u128(34),
                    tenant_id: TenantId::new(Uuid::from_u128(1)),
                    area_id: AreaId::new(Uuid::from_u128(5)),
                    claim: "sensitive result".into(),
                    evidence: vec![],
                })
                .with_rules(vec![allow, block]),
        );
        let s = McpService::new(backend, vec![]).unwrap();
        let response = s.handle_json(&json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"recall","arguments":{"context":c(),"query":"sensitive"}}}).to_string()).await;
        assert!(response.contains("disclosure blocked"));
        assert!(!response.contains("sensitive result"));
    }
}
