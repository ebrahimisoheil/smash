//! Framework-free domain and application contracts.
//!
//! `smash-core` owns policy, authorization decisions, application errors, and
//! ports. Adapters in API, worker, and storage depend on these contracts; core
//! never depends on a web framework, database driver, or async runtime.
#![forbid(unsafe_code)]

use async_trait::async_trait;
use smash_contracts::{
    AgentIdentityId, EventId, OperationId, RuleEffect, RuleId, RuleVersionId, TenantId,
};
use std::fmt;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Compatibility marker used by the Phase A storage scaffold. Real adapters
/// depend on the ports below; this value is not a business service.
pub fn core_crate_placeholder() -> &'static str {
    "smash-contracts"
}

/// Stable machine-readable application error codes. The HTTP mapping belongs
/// to the API adapter; this enum is transport-independent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    Unauthenticated,
    Forbidden,
    TenantNotFound,
    ResourceNotFound,
    InvalidRequest,
    IdempotencyConflict,
    VersionConflict,
    PolicyBlocked,
    ApprovalRequired,
    SourceQuarantined,
    OperationNotFound,
    OperationFailed,
    DependencyUnavailable,
    InternalUnexpected,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unauthenticated => "auth.unauthenticated",
            Self::Forbidden => "auth.forbidden",
            Self::TenantNotFound => "tenant.not_found",
            Self::ResourceNotFound => "resource.not_found",
            Self::InvalidRequest => "request.invalid",
            Self::IdempotencyConflict => "request.idempotency_conflict",
            Self::VersionConflict => "resource.version_conflict",
            Self::PolicyBlocked => "policy.blocked",
            Self::ApprovalRequired => "policy.approval_required",
            Self::SourceQuarantined => "source.quarantined",
            Self::OperationNotFound => "operation.not_found",
            Self::OperationFailed => "operation.failed",
            Self::DependencyUnavailable => "dependency.unavailable",
            Self::InternalUnexpected => "internal.unexpected",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One application error type. Adapters map it once to their transport body.
#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("authentication is required")]
    Unauthenticated,
    #[error("the caller is not authorized for this operation")]
    Forbidden,
    #[error("tenant is not visible")]
    TenantNotFound,
    #[error("resource is not visible or does not exist")]
    ResourceNotFound { resource: &'static str },
    #[error("request is invalid: {message}")]
    InvalidRequest { message: String },
    #[error("idempotency key was reused with a different command")]
    IdempotencyConflict,
    #[error("{resource} changed; reconcile with the current version")]
    VersionConflict {
        resource: &'static str,
        current_version: u64,
    },
    #[error("policy blocked the operation: {rationale}")]
    PolicyBlocked {
        rule_id: RuleId,
        rule_version: RuleVersionId,
        rationale: String,
    },
    #[error("policy approval is required: {rationale}")]
    ApprovalRequired {
        rule_id: RuleId,
        rule_version: RuleVersionId,
        rationale: String,
    },
    #[error("source is quarantined")]
    SourceQuarantined,
    #[error("operation is not visible or does not exist")]
    OperationNotFound,
    #[error("operation failed: {message}")]
    OperationFailed { message: String },
    #[error("dependency is unavailable: {dependency}")]
    DependencyUnavailable { dependency: &'static str },
    #[error("unexpected internal error")]
    InternalUnexpected,
}

impl ApplicationError {
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Unauthenticated => ErrorCode::Unauthenticated,
            Self::Forbidden => ErrorCode::Forbidden,
            Self::TenantNotFound => ErrorCode::TenantNotFound,
            Self::ResourceNotFound { .. } => ErrorCode::ResourceNotFound,
            Self::InvalidRequest { .. } => ErrorCode::InvalidRequest,
            Self::IdempotencyConflict => ErrorCode::IdempotencyConflict,
            Self::VersionConflict { .. } => ErrorCode::VersionConflict,
            Self::PolicyBlocked { .. } => ErrorCode::PolicyBlocked,
            Self::ApprovalRequired { .. } => ErrorCode::ApprovalRequired,
            Self::SourceQuarantined => ErrorCode::SourceQuarantined,
            Self::OperationNotFound => ErrorCode::OperationNotFound,
            Self::OperationFailed { .. } => ErrorCode::OperationFailed,
            Self::DependencyUnavailable { .. } => ErrorCode::DependencyUnavailable,
            Self::InternalUnexpected => ErrorCode::InternalUnexpected,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionToken(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdempotencyKey {
    pub tenant_id: TenantId,
    pub principal_id: Option<AgentIdentityId>,
    pub scope: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyResult {
    pub effect: RuleEffect,
    pub rule_id: RuleId,
    pub rule_version: RuleVersionId,
    pub rationale: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainEvent {
    pub event_id: EventId,
    pub tenant_id: TenantId,
    pub actor_id: Option<Uuid>,
    pub agent_identity_id: Option<AgentIdentityId>,
    pub session_id: Option<Uuid>,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub previous_version: Option<VersionToken>,
    pub resulting_version: Option<VersionToken>,
    pub reason: Option<String>,
    pub policy_result: Option<PolicyResult>,
    pub request_id: String,
    pub idempotency_key: Option<String>,
    pub occurred_at: OffsetDateTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mutation<T> {
    pub value: T,
    pub event: DomainEvent,
}

impl<T> Mutation<T> {
    pub fn new(value: T, event: DomainEvent) -> Self {
        Self { value, event }
    }
}

#[async_trait]
pub trait Repository: Send + Sync {
    async fn append_event(&self, event: &DomainEvent) -> Result<(), ApplicationError>;
    async fn claim_idempotency(&self, key: &IdempotencyKey) -> Result<(), ApplicationError>;
    async fn current_version(
        &self,
        tenant_id: TenantId,
        resource_id: Uuid,
    ) -> Result<VersionToken, ApplicationError>;
}

#[async_trait]
pub trait JobQueue: Send + Sync {
    async fn enqueue(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
    ) -> Result<(), ApplicationError>;
    async fn lease(&self, tenant_id: TenantId) -> Result<Option<OperationId>, ApplicationError>;
    async fn renew(
        &self,
        operation_id: OperationId,
        lease_token: &str,
    ) -> Result<(), ApplicationError>;
    async fn complete(
        &self,
        operation_id: OperationId,
        lease_token: &str,
    ) -> Result<(), ApplicationError>;
}

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put(
        &self,
        tenant_id: TenantId,
        object_key: &str,
        bytes: &[u8],
    ) -> Result<(), ApplicationError>;
    async fn delete(&self, tenant_id: TenantId, object_key: &str) -> Result<(), ApplicationError>;
}

pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

pub trait IdGenerator: Send + Sync {
    fn uuid_v7(&self) -> Uuid;
}

pub trait Authorization: Send + Sync {
    fn authorize(
        &self,
        tenant_id: TenantId,
        action: &str,
        target_type: &str,
    ) -> Result<(), ApplicationError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_unique() {
        let codes = [
            ErrorCode::Unauthenticated,
            ErrorCode::Forbidden,
            ErrorCode::TenantNotFound,
            ErrorCode::ResourceNotFound,
            ErrorCode::InvalidRequest,
            ErrorCode::IdempotencyConflict,
            ErrorCode::VersionConflict,
            ErrorCode::PolicyBlocked,
            ErrorCode::ApprovalRequired,
            ErrorCode::SourceQuarantined,
            ErrorCode::OperationNotFound,
            ErrorCode::OperationFailed,
            ErrorCode::DependencyUnavailable,
            ErrorCode::InternalUnexpected,
        ];
        for (index, code) in codes.iter().enumerate() {
            assert!(!codes[..index]
                .iter()
                .any(|previous| previous.as_str() == code.as_str()));
        }
    }

    #[test]
    fn mutation_carries_its_event() {
        let event = DomainEvent {
            event_id: EventId::new_v7(),
            tenant_id: TenantId::new_v7(),
            actor_id: None,
            agent_identity_id: None,
            session_id: None,
            action: "create".into(),
            target_type: "memory".into(),
            target_id: "memory-1".into(),
            previous_version: None,
            resulting_version: Some(VersionToken(1)),
            reason: None,
            policy_result: None,
            request_id: "req-1".into(),
            idempotency_key: Some("idem-1".into()),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
        };
        let mutation = Mutation::new("value", event.clone());
        assert_eq!(mutation.event, event);
    }
}
