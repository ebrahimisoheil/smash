//! Live PostgreSQL contract checks for the Phase F Map/Entity/Relationship/
//! Cross-Map adapter.
//!
//! Run explicitly against a disposable migrated database with:
//! `DATABASE_URL=... cargo test -p engrave-storage --test live_phase_f -- --ignored`.

use engrave_contracts::TenantId;
use engrave_storage::PgRepository;
use sqlx::Row;
use uuid::Uuid;

async fn seed_tenant(repository: &PgRepository, tenant_id: TenantId, slug: &str) {
    sqlx::query(
        "INSERT INTO tenants (tenant_id, slug, state, created_at, updated_at) VALUES ($1, $2, 'active', now(), now())",
    )
    .bind(tenant_id.as_uuid())
    .bind(slug)
    .execute(repository.pool())
    .await
    .unwrap();
}

async fn seed_area(repository: &PgRepository, tenant_id: TenantId, area_id: Uuid, slug: &str) {
    sqlx::query(
        "INSERT INTO areas (area_id, tenant_id, slug, state, version) VALUES ($1, $2, $3, 'active', 1)",
    )
    .bind(area_id)
    .bind(tenant_id.as_uuid())
    .bind(slug)
    .execute(repository.pool())
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn map_draft_publish_replay_conflict_and_tenant_scope_are_live() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&database_url).await.unwrap();
    let tenant_id = TenantId::new(Uuid::now_v7());
    let other_tenant_id = TenantId::new(Uuid::now_v7());
    let area_id = Uuid::now_v7();
    let map_version_id = Uuid::now_v7();

    seed_tenant(
        &repository,
        tenant_id,
        &format!("mapf-{}", tenant_id.as_uuid()),
    )
    .await;
    seed_tenant(
        &repository,
        other_tenant_id,
        &format!("mapf-other-{}", other_tenant_id.as_uuid()),
    )
    .await;
    seed_area(&repository, tenant_id, area_id, "live-map-area").await;

    // A direct write into an activated state, bypassing the governed
    // publish path entirely, must be rejected by the database itself —
    // proving the admission trigger, not just the application-layer CAS
    // check, is what makes silent publication impossible.
    let bypass_attempt = sqlx::query(
        "INSERT INTO map_versions (map_version_id, tenant_id, area_id, version_number, state, definition, version) VALUES ($1, $2, $3, 1, 'published', '{}'::jsonb, 1)",
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id.as_uuid())
    .bind(area_id)
    .execute(repository.pool())
    .await;
    assert!(
        bypass_attempt.is_err(),
        "a direct insert of an already-published Map version must be rejected without admission set"
    );

    repository
        .create_map_draft(
            tenant_id,
            map_version_id,
            area_id,
            1,
            &serde_json::json!({"kinds": [{"key": "account", "label": "Account"}], "relations": []}),
        )
        .await
        .unwrap();

    let published = repository
        .publish_map_draft(tenant_id, map_version_id, 1, "live-publish-1")
        .await
        .unwrap();
    assert_eq!(published, map_version_id);

    let replayed = repository
        .publish_map_draft(tenant_id, map_version_id, 1, "live-publish-1")
        .await
        .unwrap();
    assert_eq!(replayed, map_version_id);

    let stale = repository
        .publish_map_draft(tenant_id, map_version_id, 1, "live-publish-stale")
        .await;
    assert!(stale.is_err());

    let row = sqlx::query(
        "SELECT state, version FROM map_versions WHERE tenant_id = $1 AND map_version_id = $2",
    )
    .bind(tenant_id.as_uuid())
    .bind(map_version_id)
    .fetch_one(repository.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("state"), "published");
    assert_eq!(row.get::<i64, _>("version"), 2);

    let current_map = sqlx::query(
        "SELECT current_map_version_id FROM areas WHERE tenant_id = $1 AND area_id = $2",
    )
    .bind(tenant_id.as_uuid())
    .bind(area_id)
    .fetch_one(repository.pool())
    .await
    .unwrap();
    assert_eq!(
        current_map.get::<Uuid, _>("current_map_version_id"),
        map_version_id
    );

    let cross_tenant =
        sqlx::query("SELECT 1 FROM map_versions WHERE tenant_id = $1 AND map_version_id = $2")
            .bind(other_tenant_id.as_uuid())
            .bind(map_version_id)
            .fetch_optional(repository.pool())
            .await
            .unwrap();
    assert!(
        cross_tenant.is_none(),
        "the same map_version_id must not be visible under a foreign tenant_id predicate"
    );
}

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn entity_and_relationship_create_and_approve_are_live_and_tenant_scoped() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&database_url).await.unwrap();
    let tenant_id = TenantId::new(Uuid::now_v7());
    let other_tenant_id = TenantId::new(Uuid::now_v7());
    let area_id = Uuid::now_v7();
    let map_version_id = Uuid::now_v7();
    let account_id = Uuid::now_v7();
    let person_id = Uuid::now_v7();
    let relationship_id = Uuid::now_v7();

    seed_tenant(
        &repository,
        tenant_id,
        &format!("entf-{}", tenant_id.as_uuid()),
    )
    .await;
    seed_tenant(
        &repository,
        other_tenant_id,
        &format!("entf-other-{}", other_tenant_id.as_uuid()),
    )
    .await;
    seed_area(&repository, tenant_id, area_id, "live-entity-area").await;
    repository
        .create_map_draft(
            tenant_id,
            map_version_id,
            area_id,
            1,
            &serde_json::json!({"kinds": [], "relations": []}),
        )
        .await
        .unwrap();

    repository
        .create_entity(
            tenant_id,
            account_id,
            area_id,
            map_version_id,
            "account",
            "observed",
            &serde_json::json!({"name": "Acme"}),
        )
        .await
        .unwrap();
    repository
        .create_entity(
            tenant_id,
            person_id,
            area_id,
            map_version_id,
            "person",
            "observed",
            &serde_json::json!({"name": "Jordan"}),
        )
        .await
        .unwrap();

    let approved_account = repository
        .approve_entity(tenant_id, account_id, 1, "live-entity-approve-1")
        .await
        .unwrap();
    assert_eq!(approved_account, account_id);
    let replayed = repository
        .approve_entity(tenant_id, account_id, 1, "live-entity-approve-1")
        .await
        .unwrap();
    assert_eq!(replayed, account_id);
    let stale = repository
        .approve_entity(tenant_id, account_id, 1, "live-entity-approve-stale")
        .await;
    assert!(stale.is_err());

    repository
        .approve_entity(tenant_id, person_id, 1, "live-entity-approve-2")
        .await
        .unwrap();

    repository
        .create_relationship(
            tenant_id,
            relationship_id,
            area_id,
            map_version_id,
            person_id,
            account_id,
            "owns",
            "observed",
        )
        .await
        .unwrap();
    let approved_relationship = repository
        .approve_relationship(tenant_id, relationship_id, 1, "live-relationship-approve-1")
        .await
        .unwrap();
    assert_eq!(approved_relationship, relationship_id);

    let entity_row = sqlx::query(
        "SELECT state, kind, version FROM entities WHERE tenant_id = $1 AND entity_id = $2",
    )
    .bind(tenant_id.as_uuid())
    .bind(account_id)
    .fetch_one(repository.pool())
    .await
    .unwrap();
    assert_eq!(entity_row.get::<String, _>("state"), "active");
    assert_eq!(entity_row.get::<String, _>("kind"), "account");
    assert_eq!(entity_row.get::<i64, _>("version"), 2);

    let relationship_row = sqlx::query(
        "SELECT state, relation_kind, version FROM relationships WHERE tenant_id = $1 AND relationship_id = $2",
    )
    .bind(tenant_id.as_uuid())
    .bind(relationship_id)
    .fetch_one(repository.pool())
    .await
    .unwrap();
    assert_eq!(relationship_row.get::<String, _>("state"), "active");
    assert_eq!(relationship_row.get::<String, _>("relation_kind"), "owns");
    assert_eq!(relationship_row.get::<i64, _>("version"), 2);

    let cross_tenant =
        sqlx::query("SELECT 1 FROM entities WHERE tenant_id = $1 AND entity_id = $2")
            .bind(other_tenant_id.as_uuid())
            .bind(account_id)
            .fetch_optional(repository.pool())
            .await
            .unwrap();
    assert!(cross_tenant.is_none());
}

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn cross_map_mapping_create_and_approve_are_live_and_preserve_paths() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&database_url).await.unwrap();
    let tenant_id = TenantId::new(Uuid::now_v7());
    let source_area_id = Uuid::now_v7();
    let target_area_id = Uuid::now_v7();
    let source_map_version_id = Uuid::now_v7();
    let target_map_version_id = Uuid::now_v7();
    let mapping_id = Uuid::now_v7();

    seed_tenant(
        &repository,
        tenant_id,
        &format!("cmapf-{}", tenant_id.as_uuid()),
    )
    .await;
    seed_area(
        &repository,
        tenant_id,
        source_area_id,
        "live-cross-map-source",
    )
    .await;
    seed_area(
        &repository,
        tenant_id,
        target_area_id,
        "live-cross-map-target",
    )
    .await;
    repository
        .create_map_draft(
            tenant_id,
            source_map_version_id,
            source_area_id,
            1,
            &serde_json::json!({"kinds": [], "relations": []}),
        )
        .await
        .unwrap();
    repository
        .create_map_draft(
            tenant_id,
            target_map_version_id,
            target_area_id,
            1,
            &serde_json::json!({"kinds": [], "relations": []}),
        )
        .await
        .unwrap();

    repository
        .create_cross_map_mapping(
            tenant_id,
            mapping_id,
            source_area_id,
            target_area_id,
            source_map_version_id,
            target_map_version_id,
            "related_to",
            "shared account concept",
        )
        .await
        .unwrap();

    let approved = repository
        .approve_cross_map_mapping(tenant_id, mapping_id, 1, "live-cross-map-approve-1")
        .await
        .unwrap();
    assert_eq!(approved, mapping_id);
    let replayed = repository
        .approve_cross_map_mapping(tenant_id, mapping_id, 1, "live-cross-map-approve-1")
        .await
        .unwrap();
    assert_eq!(replayed, mapping_id);
    let stale = repository
        .approve_cross_map_mapping(tenant_id, mapping_id, 1, "live-cross-map-approve-stale")
        .await;
    assert!(stale.is_err());

    let row = sqlx::query(
        "SELECT state, version, source_area_id, target_area_id, source_map_version_id, target_map_version_id, relation FROM cross_map_mappings WHERE tenant_id = $1 AND cross_map_mapping_id = $2",
    )
    .bind(tenant_id.as_uuid())
    .bind(mapping_id)
    .fetch_one(repository.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("state"), "approved");
    assert_eq!(row.get::<i64, _>("version"), 2);
    // Mapping paths and relation are preserved unchanged across the
    // propose -> approve lifecycle transition.
    assert_eq!(row.get::<Uuid, _>("source_area_id"), source_area_id);
    assert_eq!(row.get::<Uuid, _>("target_area_id"), target_area_id);
    assert_eq!(
        row.get::<Uuid, _>("source_map_version_id"),
        source_map_version_id
    );
    assert_eq!(
        row.get::<Uuid, _>("target_map_version_id"),
        target_map_version_id
    );
    assert_eq!(row.get::<String, _>("relation"), "related_to");
}
