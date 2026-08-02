//! `smash-worker` — the Tokio job loop over `smash-core` and `smash-storage`.
//!
use async_trait::async_trait;
use smash_contracts::{OperationId, TenantId};
use smash_core::{ApplicationError, JobQueue};

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
    let _ = job_loop_once(&NoopQueue, TenantId::new_v7()).await;
    println!("smash-worker: queue loop ready");
}
