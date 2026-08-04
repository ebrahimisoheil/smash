//! Phase H live connector persistence checks.
//! Run with `DATABASE_URL=... cargo test -p engrave-storage --test live_phase_h -- --ignored`.

use engrave_contracts::{AreaId, TenantId};
use engrave_storage::PgRepository;
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn connector_source_sync_is_idempotent_and_versions_content() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&url).await.unwrap();
    let tenant = TenantId::new(Uuid::now_v7());
    let area = AreaId::new(Uuid::now_v7());
    sqlx::query("INSERT INTO tenants (tenant_id,slug,state,created_at,updated_at) VALUES ($1,$2,'active',now(),now())")
        .bind(tenant.as_uuid()).bind(format!("phase-h-{}", tenant.as_uuid())).execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO areas (area_id,tenant_id,slug,state,version) VALUES ($1,$2,'connector','active',1)")
        .bind(area.as_uuid()).bind(tenant.as_uuid()).execute(repository.pool()).await.unwrap();
    let permissions = serde_json::json!(["area:connector"]);
    let first = repository
        .queue_connector_source(
            tenant,
            area,
            "notion-source",
            "page-1",
            "Page",
            "v1",
            &permissions,
            "sync-1",
        )
        .await
        .unwrap();
    let replay = repository
        .queue_connector_source(
            tenant,
            area,
            "notion-source",
            "page-1",
            "Page",
            "v1",
            &permissions,
            "sync-1",
        )
        .await
        .unwrap();
    assert_eq!(first, replay);
    repository
        .queue_connector_source(
            tenant,
            area,
            "notion-source",
            "page-1",
            "Page",
            "v2",
            &permissions,
            "sync-2",
        )
        .await
        .unwrap();
    let versions = sqlx::query("SELECT version_number FROM source_versions sv JOIN sources s ON s.source_id=sv.source_id WHERE s.tenant_id=$1 AND s.connector_name='notion-source' AND s.external_id='page-1' ORDER BY version_number")
        .bind(tenant.as_uuid()).fetch_all(repository.pool()).await.unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].get::<i64, _>("version_number"), 1);
    assert_eq!(versions[1].get::<i64, _>("version_number"), 2);
}
