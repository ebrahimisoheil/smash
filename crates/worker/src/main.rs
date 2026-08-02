//! `smash-worker` — the Tokio job loop over `smash-core` and `smash-storage`.
//!
use async_trait::async_trait;
use smash_contracts::{OperationId, TenantId};
use smash_core::{ApplicationError, JobQueue};
use smash_storage::{wait_for_tcp_endpoint, PgRepository};

struct NoopQueue;

#[async_trait]
impl JobQueue for NoopQueue {
    async fn enqueue(
        &self,
        _tenant_id: TenantId,
        _operation_id: OperationId,
    ) -> Result<(), ApplicationError> {
        Ok(())
    }
    async fn lease(&self, _tenant_id: TenantId) -> Result<Option<OperationId>, ApplicationError> {
        Ok(None)
    }
    async fn renew(
        &self,
        _operation_id: OperationId,
        _lease_token: &str,
    ) -> Result<(), ApplicationError> {
        Ok(())
    }
    async fn complete(
        &self,
        _operation_id: OperationId,
        _lease_token: &str,
    ) -> Result<(), ApplicationError> {
        Ok(())
    }
}

async fn job_loop_once(queue: &impl JobQueue, tenant_id: TenantId) -> Result<(), ApplicationError> {
    let Some(operation_id) = queue.lease(tenant_id).await? else {
        return Ok(());
    };
    queue.complete(operation_id, "noop-lease").await
}

#[tokio::main]
async fn main() {
    let database_url = std::env::var("SMASH_DATABASE_URL").expect("SMASH_DATABASE_URL is required");
    let object_store_endpoint =
        std::env::var("SMASH_MINIO_ENDPOINT").expect("SMASH_MINIO_ENDPOINT is required");
    PgRepository::connect(&database_url)
        .await
        .expect("PostgreSQL is not ready; migrations must complete first");
    wait_for_tcp_endpoint(&object_store_endpoint, 20)
        .await
        .expect("MinIO is not ready");
    let _ = job_loop_once(&NoopQueue, TenantId::new_v7()).await;
    println!("smash-worker: queue loop ready");
}
