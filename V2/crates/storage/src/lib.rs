//! SQLx/PostgreSQL and S3-compatible object-store adapters.
//!
//! The adapters keep framework concerns out of the storage boundary. SQLx
//! queries are runtime queries until the committed offline cache is generated
//! against the canonical migration; the migration itself remains the source
//! of truth for schema shape.
#![forbid(unsafe_code)]

use async_trait::async_trait;
use aws_sdk_s3::{primitives::ByteStream, Client as S3Client};
use smash_contracts::{OperationId, OperationState, TenantId};
use smash_core::{
    ApplicationError, DomainEvent, IdempotencyKey, ObjectStore, Repository, VersionToken,
};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

/// A PostgreSQL adapter with a small, explicit repository surface.
#[derive(Clone, Debug)]
pub struct PgRepository {
    pool: PgPool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableLease {
    pub operation_id: OperationId,
    pub lease_token: String,
    pub attempt: u32,
    pub checkpoint: Option<serde_json::Value>,
    pub payload: serde_json::Value,
}

impl PgRepository {
    pub async fn connect(database_url: &str) -> Result<Self, ApplicationError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn ping(&self) -> Result<(), ApplicationError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Atomically claims queued work or work whose lease expired. The state
    /// predicate and token update happen in one statement, so two workers
    /// cannot receive the same lease.
    pub async fn claim_operation(
        &self,
        tenant_id: TenantId,
        lease_token: &str,
        lease_seconds: i64,
    ) -> Result<Option<DurableLease>, ApplicationError> {
        let row = sqlx::query(
            "UPDATE operations SET state = 'running', attempt = attempt + 1,
                 lease_token = $2, lease_expires_at = now() + make_interval(secs => $3),
                 updated_at = now()
             WHERE operation_id = (
                 SELECT operation_id FROM operations
                 WHERE tenant_id = $1 AND cancel_requested = false
                   AND (state = 'queued' OR (state IN ('leased', 'running') AND lease_expires_at <= now()))
                   AND attempt < max_attempts
                 ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1
             )
             RETURNING operation_id, attempt, checkpoint, payload",
        )
        .bind(tenant_id.as_uuid())
        .bind(lease_token)
        .bind(lease_seconds.max(1))
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(row.map(|row| DurableLease {
            operation_id: OperationId::from(row.get::<Uuid, _>("operation_id")),
            lease_token: lease_token.to_owned(),
            attempt: row.get::<i32, _>("attempt") as u32,
            checkpoint: row.try_get("checkpoint").ok(),
            payload: row.get("payload"),
        }))
    }

    pub async fn enqueue_operation(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        payload: &serde_json::Value,
        scope: &str,
        key: &str,
        max_attempts: i32,
    ) -> Result<OperationId, ApplicationError> {
        let row = sqlx::query(
            "INSERT INTO operations (operation_id, tenant_id, state, max_attempts, payload, idempotency_scope, idempotency_key, created_at, updated_at)
             VALUES ($1, $2, 'queued', $6, $3, $4, $5, now(), now())
             ON CONFLICT (tenant_id, idempotency_scope, idempotency_key)
             WHERE idempotency_scope IS NOT NULL AND idempotency_key IS NOT NULL
             DO UPDATE SET operation_id = operations.operation_id
             RETURNING operation_id",
        )
        .bind(operation_id.as_uuid()).bind(tenant_id.as_uuid()).bind(payload)
        .bind(scope).bind(key).bind(max_attempts.max(1))
        .fetch_one(&self.pool).await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(OperationId::from(row.get::<Uuid, _>("operation_id")))
    }

    pub async fn update_source_state(
        &self,
        tenant_id: TenantId,
        source_id: Uuid,
        source_version_id: Option<Uuid>,
        state: &str,
        quarantine_reason: Option<&str>,
    ) -> Result<(), ApplicationError> {
        let result = sqlx::query(
            "UPDATE sources SET state = $3, updated_at = now(), version = version + 1
             WHERE tenant_id = $1 AND source_id = $2",
        )
        .bind(tenant_id.as_uuid())
        .bind(source_id)
        .bind(state)
        .execute(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::ResourceNotFound { resource: "source" });
        }
        if let (Some(version_id), Some(reason)) = (source_version_id, quarantine_reason) {
            sqlx::query("UPDATE source_versions SET state = 'quarantined', quarantine_reason = $3 WHERE tenant_id = $1 AND source_version_id = $2")
                .bind(tenant_id.as_uuid()).bind(version_id).bind(reason).execute(&self.pool).await
                .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn persist_text_output(
        &self,
        tenant_id: TenantId,
        source_version_id: Uuid,
        artifact_id: Uuid,
        processor: &str,
        processor_version: &str,
        input_hash: &str,
        chunks: &[(Uuid, &str, &str, &str, &str)],
        warnings: &[String],
    ) -> Result<(), ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        sqlx::query("INSERT INTO artifacts (artifact_id, tenant_id, source_version_id, processor_name, processor_version, input_hash, state, descriptor, warnings) VALUES ($1, $2, $3, $4, $5, $6, 'available', $7, $8) ON CONFLICT (source_version_id, processor_name, processor_version, input_hash) DO NOTHING")
            .bind(artifact_id).bind(tenant_id.as_uuid()).bind(source_version_id).bind(processor).bind(processor_version).bind(input_hash)
            .bind(serde_json::json!({"kind":"text"})).bind(serde_json::json!(warnings)).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        for (chunk_id, representation, coordinate, content_hash, content) in chunks {
            sqlx::query("INSERT INTO chunks (chunk_id, tenant_id, source_version_id, artifact_id, representation, coordinate, content_hash, content, state) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'created') ON CONFLICT (source_version_id, representation, coordinate, content_hash) DO NOTHING")
                .bind(chunk_id).bind(tenant_id.as_uuid()).bind(source_version_id).bind(artifact_id).bind(representation).bind(coordinate).bind(content_hash).bind(content).execute(&mut *tx).await
                .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        }
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start_processor_run(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        source_version_id: Uuid,
        processor_run_id: Uuid,
        processor_name: &str,
        processor_version: &str,
        configuration_fingerprint: &str,
        input_hash: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO processor_runs (processor_run_id, tenant_id, operation_id, source_version_id, processor_name, processor_version, configuration_fingerprint, input_hash, state, started_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'running', now()) ON CONFLICT (processor_run_id) DO NOTHING")
            .bind(processor_run_id).bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).bind(source_version_id).bind(processor_name).bind(processor_version).bind(configuration_fingerprint).bind(input_hash).execute(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(())
    }

    pub async fn finish_processor_run(
        &self,
        tenant_id: TenantId,
        processor_run_id: Uuid,
        state: &str,
        warnings: &[String],
    ) -> Result<(), ApplicationError> {
        sqlx::query("UPDATE processor_runs SET state = $3, warnings = $4, finished_at = now() WHERE tenant_id = $1 AND processor_run_id = $2")
            .bind(tenant_id.as_uuid()).bind(processor_run_id).bind(state).bind(serde_json::json!(warnings)).execute(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(())
    }

    pub async fn append_process_evidence(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        evidence_id: Uuid,
        kind: &str,
        payload: &serde_json::Value,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO process_evidence (process_evidence_id, tenant_id, operation_id, kind, payload, created_at) VALUES ($1, $2, $3, $4, $5, now()) ON CONFLICT (process_evidence_id) DO NOTHING")
            .bind(evidence_id).bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).bind(kind).bind(payload).execute(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(())
    }

    pub async fn save_checkpoint(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        lease_token: &str,
        key: &str,
        payload: &serde_json::Value,
        progress: i16,
    ) -> Result<(), ApplicationError> {
        let mut transaction =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        let result = sqlx::query(
            "UPDATE operations
             SET progress = $4,
                 checkpoint = jsonb_build_object('key', $5::text, 'payload', $6::jsonb),
                 updated_at = now()
             WHERE operation_id = $1 AND tenant_id = $2 AND lease_token = $3 AND state = 'running'",
        )
        .bind(operation_id.as_uuid())
        .bind(tenant_id.as_uuid())
        .bind(lease_token)
        .bind(progress.clamp(0, 100))
        .bind(key)
        .bind(payload)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        sqlx::query(
            "INSERT INTO operation_checkpoints (operation_id, tenant_id, checkpoint_key, payload, progress, created_at)
             VALUES ($1, $2, $3, $4, $5, now())
             ON CONFLICT (operation_id, checkpoint_key) DO UPDATE
             SET payload = EXCLUDED.payload, progress = EXCLUDED.progress, created_at = EXCLUDED.created_at",
        )
        .bind(operation_id.as_uuid())
        .bind(tenant_id.as_uuid())
        .bind(key)
        .bind(payload)
        .bind(progress.clamp(0, 100))
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        transaction
            .commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    pub async fn request_operation_cancel(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
    ) -> Result<(), ApplicationError> {
        let result = sqlx::query(
            "UPDATE operations SET cancel_requested = true,
                 state = CASE WHEN state = 'queued' THEN 'cancelled' ELSE state END, updated_at = now()
             WHERE tenant_id = $1 AND operation_id = $2 AND state NOT IN ('succeeded', 'failed', 'cancelled')",
        ).bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).execute(&self.pool).await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        Ok(())
    }

    pub async fn is_cancel_requested(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
    ) -> Result<bool, ApplicationError> {
        sqlx::query(
            "SELECT cancel_requested FROM operations WHERE tenant_id = $1 AND operation_id = $2",
        )
        .bind(tenant_id.as_uuid())
        .bind(operation_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?
        .map(|row| row.get("cancel_requested"))
        .ok_or(ApplicationError::OperationNotFound)
    }

    pub async fn fail_operation(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        lease_token: &str,
        error_code: &str,
        error_message: &str,
    ) -> Result<(), ApplicationError> {
        let result = sqlx::query("UPDATE operations SET state = CASE WHEN cancel_requested THEN 'cancelled' WHEN attempt < max_attempts THEN 'queued' ELSE 'failed' END, error_code = $4, error_message = $5, lease_token = NULL, lease_expires_at = NULL, updated_at = now() WHERE tenant_id = $1 AND operation_id = $2 AND lease_token = $3")
            .bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).bind(lease_token).bind(error_code).bind(error_message)
            .execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        Ok(())
    }

    pub async fn finish_operation(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        lease_token: &str,
        state: OperationState,
        error: Option<&str>,
    ) -> Result<(), ApplicationError> {
        let state = match state {
            OperationState::Succeeded => "succeeded",
            OperationState::Failed => "failed",
            OperationState::Cancelled => "cancelled",
            _ => {
                return Err(ApplicationError::InvalidRequest {
                    message: "operation cannot finish in this state".into(),
                })
            }
        };
        let result = sqlx::query("UPDATE operations SET state = $4, lease_token = NULL, lease_expires_at = NULL, progress = CASE WHEN $4 = 'succeeded' THEN 100 ELSE progress END, error_message = $5, updated_at = now() WHERE tenant_id = $1 AND operation_id = $2 AND lease_token = $3")
            .bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).bind(lease_token).bind(state).bind(error)
            .execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        Ok(())
    }
}

#[async_trait]
impl Repository for PgRepository {
    async fn append_event(&self, event: &DomainEvent) -> Result<(), ApplicationError> {
        sqlx::query(
            "INSERT INTO events
             (event_id, tenant_id, actor_id, agent_identity_id, session_id, action,
              target_type, target_id, previous_version, resulting_version, reason,
              request_id, idempotency_key, occurred_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
        )
        .bind(event.event_id.as_uuid())
        .bind(event.tenant_id.as_uuid())
        .bind(event.actor_id)
        .bind(event.agent_identity_id.map(Into::<Uuid>::into))
        .bind(event.session_id)
        .bind(&event.action)
        .bind(&event.target_type)
        .bind(&event.target_id)
        .bind(event.previous_version.as_ref().map(|v| v.0 as i64))
        .bind(event.resulting_version.as_ref().map(|v| v.0 as i64))
        .bind(&event.reason)
        .bind(&event.request_id)
        .bind(&event.idempotency_key)
        .bind(event.occurred_at)
        .execute(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        Ok(())
    }

    async fn claim_idempotency(&self, key: &IdempotencyKey) -> Result<(), ApplicationError> {
        sqlx::query(
            "INSERT INTO idempotency_keys (tenant_id, scope, key, request_hash, created_at)
             VALUES ($1, $2, $3, $4, now()) ON CONFLICT (tenant_id, scope, key) DO NOTHING",
        )
        .bind(key.tenant_id.as_uuid())
        .bind(&key.scope)
        .bind(&key.value)
        .bind(
            key.principal_id
                .map(Into::<Uuid>::into)
                .map(|id| id.to_string()),
        )
        .execute(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        Ok(())
    }

    async fn current_version(
        &self,
        tenant_id: TenantId,
        resource_id: Uuid,
    ) -> Result<VersionToken, ApplicationError> {
        let row = sqlx::query(
            "SELECT version FROM entities WHERE tenant_id = $1 AND entity_id = $2
             UNION ALL SELECT version FROM memories WHERE tenant_id = $1 AND memory_id = $2
             UNION ALL SELECT version FROM proposals WHERE tenant_id = $1 AND proposal_id = $2
             LIMIT 1",
        )
        .bind(tenant_id.as_uuid())
        .bind(resource_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        row.map(|row| VersionToken(row.get::<i64, _>("version") as u64))
            .ok_or(ApplicationError::ResourceNotFound {
                resource: "versioned_resource",
            })
    }
}

/// Stable, tenant-prefixed object key. The source filename is deliberately
/// excluded so renames cannot create a second logical object.
pub fn source_object_key(tenant_id: TenantId, source_id: Uuid, source_version_id: Uuid) -> String {
    format!(
        "tenants/{}/sources/{}/versions/{}",
        tenant_id.as_uuid(),
        source_id,
        source_version_id
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizationMetadata {
    pub tenant_id: TenantId,
    pub object_key: String,
    pub expected_key: String,
    pub byte_size: u64,
    pub expected_byte_size: u64,
    pub media_type: String,
    pub expected_media_type: String,
    pub checksum: String,
    pub expected_checksum: String,
}

pub fn validate_finalization(metadata: &FinalizationMetadata) -> Result<(), ApplicationError> {
    let expected_prefix = format!("tenants/{}/", metadata.tenant_id.as_uuid());
    if !metadata.object_key.starts_with(&expected_prefix)
        || metadata.object_key != metadata.expected_key
        || metadata.byte_size != metadata.expected_byte_size
        || metadata.media_type != metadata.expected_media_type
        || metadata.checksum != metadata.expected_checksum
    {
        return Err(ApplicationError::InvalidRequest {
            message: "staged object metadata does not match the source version".to_owned(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct S3ObjectStore {
    client: S3Client,
    bucket: String,
}

impl S3ObjectStore {
    pub fn new(client: S3Client, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    pub async fn ensure_bucket(&self) -> Result<(), ApplicationError> {
        if self
            .client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .is_ok()
        {
            return Ok(());
        }
        self.client
            .create_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "object_store",
            })?;
        Ok(())
    }
}

#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put(
        &self,
        tenant_id: TenantId,
        object_key: &str,
        bytes: &[u8],
    ) -> Result<(), ApplicationError> {
        if !object_key.starts_with(&format!("tenants/{}/", tenant_id.as_uuid())) {
            return Err(ApplicationError::Forbidden);
        }
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(object_key)
            .body(ByteStream::from(bytes.to_vec()))
            .send()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "object_store",
            })?;
        Ok(())
    }

    async fn delete(&self, tenant_id: TenantId, object_key: &str) -> Result<(), ApplicationError> {
        if !object_key.starts_with(&format!("tenants/{}/", tenant_id.as_uuid())) {
            return Err(ApplicationError::Forbidden);
        }
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(object_key)
            .send()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "object_store",
            })?;
        Ok(())
    }
}

/// Compatibility marker retained from the Phase A scaffold.
pub fn storage_crate_placeholder() -> &'static str {
    "smash-contracts"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_keys_are_stable_and_tenant_scoped() {
        let tenant = TenantId::new(Uuid::nil());
        assert_eq!(
            source_object_key(tenant, Uuid::from_u128(1), Uuid::from_u128(2)),
            "tenants/00000000-0000-0000-0000-000000000000/sources/00000000-0000-0000-0000-000000000001/versions/00000000-0000-0000-0000-000000000002"
        );
    }

    #[test]
    fn finalization_rejects_mismatched_metadata() {
        let tenant = TenantId::new(Uuid::nil());
        let metadata = FinalizationMetadata {
            tenant_id: tenant,
            object_key: "tenants/00000000-0000-0000-0000-000000000000/a".into(),
            expected_key: "tenants/00000000-0000-0000-0000-000000000000/b".into(),
            byte_size: 1,
            expected_byte_size: 2,
            media_type: "text/plain".into(),
            expected_media_type: "application/pdf".into(),
            checksum: "a".into(),
            expected_checksum: "b".into(),
        };
        assert!(validate_finalization(&metadata).is_err());
    }

    #[test]
    fn links_against_core() {
        assert_eq!(storage_crate_placeholder(), "smash-contracts");
    }
}
