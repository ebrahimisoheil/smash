use engrave_contracts::{OperationId, TenantId};
use engrave_storage::PgRepository;
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn postgres_queue_claim_checkpoint_renew_cancel_and_retry_are_live() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&database_url).await.unwrap();
    let tenant_id = TenantId::new_v7();
    sqlx::query("INSERT INTO tenants (tenant_id, slug, state, created_at, updated_at) VALUES ($1, $2, 'active', now(), now())")
        .bind(tenant_id.as_uuid())
        .bind(format!("queue-{}", tenant_id.as_uuid()))
        .execute(repository.pool())
        .await
        .unwrap();

    let operation_id = OperationId::new_v7();
    repository
        .enqueue_operation(
            tenant_id,
            operation_id,
            &serde_json::json!({"kind":"embedding"}),
            "live-queue",
            &Uuid::now_v7().to_string(),
            3,
        )
        .await
        .unwrap();
    let token = "live-queue-lease-1";
    let lease = repository
        .claim_operation(tenant_id, token, 60)
        .await
        .unwrap()
        .expect("queued operation should be claimed");
    assert_eq!(lease.operation_id, operation_id);
    repository
        .save_checkpoint(
            tenant_id,
            operation_id,
            token,
            "embedding-batch-1",
            &serde_json::json!({"completed": 2}),
            40,
        )
        .await
        .unwrap();
    repository
        .renew_operation(tenant_id, operation_id, token, 120)
        .await
        .unwrap();
    let checkpoint: serde_json::Value =
        sqlx::query("SELECT checkpoint FROM operations WHERE operation_id = $1")
            .bind(operation_id.as_uuid())
            .fetch_one(repository.pool())
            .await
            .unwrap()
            .get("checkpoint");
    assert_eq!(checkpoint["key"], "embedding-batch-1");

    repository
        .fail_operation(
            tenant_id,
            operation_id,
            token,
            "provider.timeout",
            "retryable",
        )
        .await
        .unwrap();
    let retry = repository
        .claim_operation(tenant_id, "live-queue-lease-2", 60)
        .await
        .unwrap()
        .expect("retryable failure should requeue");
    assert_eq!(retry.attempt, 2);
    repository
        .request_operation_cancel(tenant_id, operation_id)
        .await
        .unwrap();
    assert!(repository
        .is_cancel_requested(tenant_id, operation_id)
        .await
        .unwrap());
    repository
        .finish_operation(
            tenant_id,
            operation_id,
            "live-queue-lease-2",
            engrave_contracts::OperationState::Cancelled,
            None,
        )
        .await
        .unwrap();
    let state: String = sqlx::query("SELECT state FROM operations WHERE operation_id = $1")
        .bind(operation_id.as_uuid())
        .fetch_one(repository.pool())
        .await
        .unwrap()
        .get("state");
    assert_eq!(state, "cancelled");
}

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn postgres_queue_reclaims_expired_leases_and_supports_manual_retry() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&database_url).await.unwrap();
    let tenant_id = TenantId::new_v7();
    sqlx::query("INSERT INTO tenants (tenant_id, slug, state, created_at, updated_at) VALUES ($1, $2, 'active', now(), now())")
        .bind(tenant_id.as_uuid())
        .bind(format!("reclaim-{}", tenant_id.as_uuid()))
        .execute(repository.pool())
        .await
        .unwrap();
    let expired_id = OperationId::new_v7();
    repository
        .enqueue_operation(
            tenant_id,
            expired_id,
            &serde_json::json!({}),
            "reclaim",
            &Uuid::now_v7().to_string(),
            3,
        )
        .await
        .unwrap();
    repository
        .claim_operation(tenant_id, "expired-1", 1)
        .await
        .unwrap()
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1_200)).await;
    let reclaimed = repository
        .claim_operation(tenant_id, "expired-2", 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reclaimed.operation_id, expired_id);
    assert_eq!(reclaimed.attempt, 2);

    let dead_id = OperationId::new_v7();
    repository
        .enqueue_operation(
            tenant_id,
            dead_id,
            &serde_json::json!({}),
            "dead-letter",
            &Uuid::now_v7().to_string(),
            1,
        )
        .await
        .unwrap();
    repository
        .claim_operation(tenant_id, "dead-1", 60)
        .await
        .unwrap()
        .unwrap();
    repository
        .fail_operation(tenant_id, dead_id, "dead-1", "provider.quota", "exhausted")
        .await
        .unwrap();
    let state: String = sqlx::query("SELECT state FROM operations WHERE operation_id = $1")
        .bind(dead_id.as_uuid())
        .fetch_one(repository.pool())
        .await
        .unwrap()
        .get("state");
    assert_eq!(state, "failed");
    repository
        .manual_retry_operation(tenant_id, dead_id)
        .await
        .unwrap();
    let retried = repository
        .claim_operation(tenant_id, "dead-2", 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retried.attempt, 1);
}
