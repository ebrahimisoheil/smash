//! Durable Phase C worker. Each processing step is persisted before the next
//! one, so a lease recovery resumes from the last checkpoint without turning
//! process observations into durable Memory.

use engrave_contracts::{OperationState, TenantId};
use engrave_core::{process_text, EmbeddingConfiguration, ProjectionIdentity};
use engrave_storage::{LanceProjectionAdapter, PgRepository};
use std::time::Duration;
use uuid::Uuid;

const LEASE_SECONDS: i64 = 60;
const MAX_BYTES: usize = 10 * 1024 * 1024;
const MAX_CHUNKS: usize = 10_000;
const RETRIEVAL_DIMENSION: usize = 32;

fn env_tenant() -> Result<TenantId, String> {
    let value = std::env::var("ENGRAVE_TENANT_ID")
        .map_err(|_| "ENGRAVE_TENANT_ID is required".to_owned())?;
    Uuid::parse_str(&value)
        .map(TenantId::new)
        .map_err(|_| "ENGRAVE_TENANT_ID must be a UUID".to_owned())
}

async fn process_once(
    repository: &PgRepository,
    tenant_id: TenantId,
    lance: Option<&LanceProjectionAdapter>,
) -> Result<bool, String> {
    let lease_token = format!("worker-{}", Uuid::now_v7());
    let Some(lease) = repository
        .claim_operation(tenant_id, &lease_token, LEASE_SECONDS)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(false);
    };
    repository
        .renew_operation(tenant_id, lease.operation_id, &lease_token, LEASE_SECONDS)
        .await
        .map_err(|e| e.to_string())?;
    let payload = lease.payload;
    let operation_kind = payload.get("kind").and_then(|value| value.as_str());
    if matches!(
        operation_kind,
        Some("embedding")
            | Some("re-embedding")
            | Some("index")
            | Some("rebuild")
            | Some("reconcile")
    ) {
        let Some(lance) = lance else {
            repository
                .fail_operation(
                    tenant_id,
                    lease.operation_id,
                    &lease_token,
                    "retrieval.lancedb_unavailable",
                    "retrieval operation requires the worker LanceDB writer",
                )
                .await
                .map_err(|error| error.to_string())?;
            return Ok(true);
        };
        if repository
            .is_cancel_requested(tenant_id, lease.operation_id)
            .await
            .map_err(|error| error.to_string())?
        {
            repository
                .finish_operation(
                    tenant_id,
                    lease.operation_id,
                    &lease_token,
                    OperationState::Cancelled,
                    Some("retrieval operation cancelled before reconciliation"),
                )
                .await
                .map_err(|error| error.to_string())?;
            return Ok(true);
        }
        repository
            .save_checkpoint(
                tenant_id,
                lease.operation_id,
                &lease_token,
                "retrieval-reconciliation-started",
                &serde_json::json!({"kind": operation_kind, "profile": std::env::var("ENGRAVE_EMBEDDING_PROFILE").ok()}),
                10,
            )
            .await
            .map_err(|error| error.to_string())?;
        match reconcile_retrieval_projection(repository, lance, tenant_id).await {
            Ok(()) => {
                if std::env::var("ENGRAVE_RETRIEVAL_INDEX").as_deref() == Ok("ann") {
                    if let Err(error) = lance.build_ann_index().await {
                        let message = error.to_string();
                        repository
                            .fail_operation(
                                tenant_id,
                                lease.operation_id,
                                &lease_token,
                                "retrieval.ann_index_failed",
                                &message,
                            )
                            .await
                            .map_err(|error| error.to_string())?;
                        return Ok(true);
                    }
                }
                repository
                    .save_checkpoint(
                        tenant_id,
                        lease.operation_id,
                        &lease_token,
                        "retrieval-reconciliation-complete",
                        &serde_json::json!({"kind": operation_kind}),
                        100,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                repository
                    .finish_operation(
                        tenant_id,
                        lease.operation_id,
                        &lease_token,
                        OperationState::Succeeded,
                        None,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
            }
            Err(error) => {
                repository
                    .fail_operation(
                        tenant_id,
                        lease.operation_id,
                        &lease_token,
                        "retrieval.reconciliation_failed",
                        &error,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
            }
        }
        return Ok(true);
    }
    let source_id = payload
        .get("source_id")
        .and_then(|v| v.as_str())
        .and_then(|v| Uuid::parse_str(v).ok());
    let source_version_id = payload
        .get("source_version_id")
        .and_then(|v| v.as_str())
        .and_then(|v| Uuid::parse_str(v).ok());
    let content = payload
        .get("content")
        .and_then(|v| v.as_str())
        .map(str::as_bytes);
    let processor_name = payload
        .get("processor_name")
        .and_then(|v| v.as_str())
        .unwrap_or("engrave-text");
    let processor_version = payload
        .get("processor_version")
        .and_then(|v| v.as_str())
        .unwrap_or("1");
    let media_type = payload
        .get("media_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text/plain");
    let (Some(source_id), Some(source_version_id), Some(content)) =
        (source_id, source_version_id, content)
    else {
        repository
            .fail_operation(
                tenant_id,
                lease.operation_id,
                &lease_token,
                "operation.invalid_payload",
                "ingest operation requires source_id, source_version_id, and UTF-8 content",
            )
            .await
            .map_err(|e| e.to_string())?;
        return Ok(true);
    };
    if !matches!(media_type, "text/plain" | "text/markdown" | "text/csv") {
        let reason = format!("unsupported media type: {media_type}");
        repository
            .update_source_state(
                tenant_id,
                source_id,
                Some(source_version_id),
                "quarantined",
                Some(&reason),
            )
            .await
            .map_err(|e| e.to_string())?;
        repository
            .fail_operation(
                tenant_id,
                lease.operation_id,
                &lease_token,
                "source.quarantined",
                &reason,
            )
            .await
            .map_err(|e| e.to_string())?;
        return Ok(true);
    }
    repository
        .update_source_state(
            tenant_id,
            source_id,
            Some(source_version_id),
            "extracting",
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
    if repository
        .is_cancel_requested(tenant_id, lease.operation_id)
        .await
        .map_err(|e| e.to_string())?
    {
        repository
            .finish_operation(
                tenant_id,
                lease.operation_id,
                &lease_token,
                OperationState::Cancelled,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        return Ok(true);
    }
    repository
        .save_checkpoint(
            tenant_id,
            lease.operation_id,
            &lease_token,
            "extracting",
            &serde_json::json!({"state":"extracting"}),
            20,
        )
        .await
        .map_err(|e| e.to_string())?;
    let output = match process_text(content, MAX_BYTES, MAX_CHUNKS) {
        Ok(output) => output,
        Err(error) => {
            repository
                .update_source_state(
                    tenant_id,
                    source_id,
                    Some(source_version_id),
                    "quarantined",
                    Some(&format!("processor rejected input: {error:?}")),
                )
                .await
                .map_err(|e| e.to_string())?;
            repository
                .fail_operation(
                    tenant_id,
                    lease.operation_id,
                    &lease_token,
                    "source.quarantined",
                    &format!("processor rejected input: {error:?}"),
                )
                .await
                .map_err(|e| e.to_string())?;
            return Ok(true);
        }
    };
    let processor_run_id = Uuid::now_v7();
    repository
        .start_processor_run(
            tenant_id,
            lease.operation_id,
            source_version_id,
            processor_run_id,
            processor_name,
            processor_version,
            "default",
            &output.input_hash,
        )
        .await
        .map_err(|e| e.to_string())?;
    repository
        .update_source_state(
            tenant_id,
            source_id,
            Some(source_version_id),
            "chunking",
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
    repository
        .save_checkpoint(
            tenant_id,
            lease.operation_id,
            &lease_token,
            "chunking",
            &serde_json::json!({"chunks":output.chunks.len()}),
            60,
        )
        .await
        .map_err(|e| e.to_string())?;
    if repository
        .is_cancel_requested(tenant_id, lease.operation_id)
        .await
        .map_err(|e| e.to_string())?
    {
        repository
            .finish_operation(
                tenant_id,
                lease.operation_id,
                &lease_token,
                OperationState::Cancelled,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        return Ok(true);
    }
    let artifact_id = Uuid::now_v7();
    let chunk_ids: Vec<Uuid> = output.chunks.iter().map(|_| Uuid::now_v7()).collect();
    let rows: Vec<(Uuid, &str, &str, &str, &str)> = output
        .chunks
        .iter()
        .zip(chunk_ids.iter())
        .map(|(chunk, id)| {
            (
                *id,
                "text",
                chunk.coordinate.as_str(),
                chunk.content_hash.as_str(),
                chunk.text.as_str(),
            )
        })
        .collect();
    repository
        .persist_text_output(
            tenant_id,
            source_version_id,
            artifact_id,
            processor_name,
            processor_version,
            &output.input_hash,
            &rows,
            &output.warnings,
        )
        .await
        .map_err(|e| e.to_string())?;
    repository
        .finish_processor_run(tenant_id, processor_run_id, "completed", &output.warnings)
        .await
        .map_err(|e| e.to_string())?;
    repository
        .update_source_state(
            tenant_id,
            source_id,
            Some(source_version_id),
            "proposing",
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
    repository.append_process_evidence(tenant_id, lease.operation_id, Uuid::now_v7(), "processor.completed", &serde_json::json!({"processor":processor_name,"processor_version":processor_version,"input_hash":output.input_hash,"artifact_id":artifact_id,"chunk_count":output.chunks.len(),"memory_activation":false})).await.map_err(|e| e.to_string())?;
    repository
        .save_checkpoint(
            tenant_id,
            lease.operation_id,
            &lease_token,
            "ready",
            &serde_json::json!({"artifact_id":artifact_id,"chunk_count":output.chunks.len()}),
            100,
        )
        .await
        .map_err(|e| e.to_string())?;
    repository
        .update_source_state(tenant_id, source_id, Some(source_version_id), "ready", None)
        .await
        .map_err(|e| e.to_string())?;
    repository
        .finish_operation(
            tenant_id,
            lease.operation_id,
            &lease_token,
            OperationState::Succeeded,
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

async fn reconcile_retrieval_projection(
    repository: &PgRepository,
    lance: &LanceProjectionAdapter,
    tenant_id: TenantId,
) -> Result<(), String> {
    let profile_name = std::env::var("ENGRAVE_EMBEDDING_PROFILE").unwrap_or_default();
    if profile_name != "deterministic-dev"
        && profile_name != "voyage-3-lite"
        && profile_name != "openai-large"
    {
        return Ok(());
    }
    let identity = if profile_name == "voyage-3-lite" || profile_name == "openai-large" {
        EmbeddingConfiguration::production_candidates()
            .map_err(|error| format!("invalid provider configuration: {error:?}"))?
            .profile(&profile_name)
            .map_err(|error| format!("missing provider profile: {error:?}"))?
            .identity()
            .map_err(|error| format!("invalid retrieval identity: {error:?}"))?
    } else {
        ProjectionIdentity::new(
            "deterministic",
            "default",
            "1",
            RETRIEVAL_DIMENSION,
            "v1",
            "default",
        )
        .map_err(|error| format!("invalid retrieval identity: {error:?}"))?
    };
    let rows = repository
        .retrieval_projection_rows(tenant_id, &identity)
        .await
        .map_err(|error| error.to_string())?;
    lance
        .reconcile(&rows)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let database_url =
        std::env::var("ENGRAVE_DATABASE_URL").expect("ENGRAVE_DATABASE_URL is required");
    let tenant_id = env_tenant().expect("invalid worker tenant configuration");
    let repository = PgRepository::connect(&database_url)
        .await
        .expect("worker cannot connect to postgres");
    let lance = match std::env::var("ENGRAVE_LANCEDB_PATH") {
        Ok(path) => Some(
            LanceProjectionAdapter::connect(&path, "memory_projection")
                .await
                .expect("worker cannot connect to LanceDB"),
        ),
        Err(_) => None,
    };
    loop {
        if let Err(error) = process_once(&repository, tenant_id, lance.as_ref()).await {
            eprintln!("engrave-worker processing error: {error}");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_limits_are_bounded() {
        assert_eq!(engrave_core::hex_hash(b"engrave").len(), 64);
        const {
            assert!(MAX_BYTES > 0 && MAX_CHUNKS > 0 && LEASE_SECONDS > 0);
        }
    }
}
