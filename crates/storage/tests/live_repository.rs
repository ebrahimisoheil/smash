//! Live PostgreSQL contract checks for the SQLx adapter.
//!
//! Run explicitly against a disposable migrated database with:
//! `DATABASE_URL=... cargo test -p engrave-storage --test live_repository -- --ignored`.

use engrave_contracts::TenantId;
use engrave_core::{ActorRole, AuthorizationContext, Repository, SearchRequest};
use engrave_storage::PgRepository;
use sqlx::types::time::OffsetDateTime;
use sqlx::Row;
use std::collections::BTreeSet;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn proposal_review_replay_conflict_and_tenant_scope_are_live() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&database_url).await.unwrap();
    let tenant_id = TenantId::new(Uuid::now_v7());
    let other_tenant_id = TenantId::new(Uuid::now_v7());
    let area_id = Uuid::now_v7();
    let proposal_id = Uuid::now_v7();
    let memory_id = Uuid::now_v7();
    let memory_version_id = Uuid::now_v7();
    let reviewer_id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO tenants (tenant_id, slug, state, created_at, updated_at) VALUES ($1, $2, 'active', now(), now()), ($3, $4, 'active', now(), now())",
    )
    .bind(tenant_id.as_uuid())
    .bind(format!("live-{}", tenant_id.as_uuid()))
    .bind(other_tenant_id.as_uuid())
    .bind(format!("live-{}", other_tenant_id.as_uuid()))
    .execute(repository.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO areas (area_id, tenant_id, slug, state, version) VALUES ($1, $2, 'live', 'active', 1)",
    )
    .bind(area_id)
    .bind(tenant_id.as_uuid())
    .execute(repository.pool())
    .await
    .unwrap();

    repository
        .create_memory_proposal(
            tenant_id,
            proposal_id,
            area_id,
            "captured",
            "claim",
            &serde_json::json!({"claim":"Use UTC","reason":"live test"}),
        )
        .await
        .unwrap();

    let approved = repository
        .approve_memory_proposal(
            tenant_id,
            proposal_id,
            reviewer_id,
            1,
            "live-review-1",
            memory_id,
            memory_version_id,
            "Use UTC",
            "area",
            "always",
            "live test",
            &serde_json::json!(["chunk:live"]),
        )
        .await
        .unwrap();
    assert_eq!(approved, memory_id);

    let replayed = repository
        .approve_memory_proposal(
            tenant_id,
            proposal_id,
            reviewer_id,
            1,
            "live-review-1",
            Uuid::now_v7(),
            Uuid::now_v7(),
            "ignored on replay",
            "area",
            "always",
            "ignored on replay",
            &serde_json::json!([]),
        )
        .await
        .unwrap();
    assert_eq!(replayed, memory_id);

    let stale = repository
        .approve_memory_proposal(
            tenant_id,
            proposal_id,
            reviewer_id,
            1,
            "live-review-stale",
            Uuid::now_v7(),
            Uuid::now_v7(),
            "missing",
            "area",
            "always",
            "missing",
            &serde_json::json!([]),
        )
        .await;
    assert!(stale.is_err());

    let state = sqlx::query(
        "SELECT state, current_version_id FROM memories WHERE tenant_id = $1 AND memory_id = $2",
    )
    .bind(tenant_id.as_uuid())
    .bind(memory_id)
    .fetch_one(repository.pool())
    .await
    .unwrap();
    assert_eq!(state.get::<String, _>("state"), "active");
    assert_eq!(
        state.get::<Uuid, _>("current_version_id"),
        memory_version_id
    );

    let cross_tenant = repository.current_version(other_tenant_id, memory_id).await;
    assert!(cross_tenant.is_err());
}

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn live_lexical_search_is_tenant_area_and_actor_scoped() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&database_url).await.unwrap();
    let tenant_id = TenantId::new(Uuid::now_v7());
    let other_tenant_id = TenantId::new(Uuid::now_v7());
    let area_id = Uuid::now_v7();
    let other_area_id = Uuid::now_v7();
    let actor_id = Uuid::now_v7();
    let other_actor_id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO tenants (tenant_id, slug, state, created_at, updated_at) VALUES ($1, $2, 'active', now(), now()), ($3, $4, 'active', now(), now())",
    )
    .bind(tenant_id.as_uuid())
    .bind(format!("search-{}", tenant_id.as_uuid()))
    .bind(other_tenant_id.as_uuid())
    .bind(format!("search-{}", other_tenant_id.as_uuid()))
    .execute(repository.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO actors (actor_id, tenant_id, issuer, subject, state) VALUES ($1, $2, 'test', $3, 'active'), ($4, $2, 'test', $5, 'active')",
    )
    .bind(actor_id)
    .bind(tenant_id.as_uuid())
    .bind(actor_id.to_string())
    .bind(other_actor_id)
    .bind(other_actor_id.to_string())
    .execute(repository.pool())
    .await
    .unwrap();
    for (id, tenant, slug) in [
        (area_id, tenant_id.as_uuid(), "search"),
        (other_area_id, other_tenant_id.as_uuid(), "search"),
    ] {
        sqlx::query(
            "INSERT INTO areas (area_id, tenant_id, slug, state, version) VALUES ($1, $2, $3, 'active', 1)",
        )
        .bind(id)
        .bind(tenant)
        .bind(format!("{}-{}", slug, id))
        .execute(repository.pool())
        .await
        .unwrap();
    }

    let mut tx = repository.pool().begin().await.unwrap();
    sqlx::query("SET LOCAL app.memory_admission = 'approved'")
        .execute(&mut *tx)
        .await
        .unwrap();
    for (memory_id, version_id, tenant, area, scope, owner, claim) in [
        (
            Uuid::now_v7(),
            Uuid::now_v7(),
            tenant_id.as_uuid(),
            area_id,
            "area",
            None,
            "renewal security policy",
        ),
        (
            Uuid::now_v7(),
            Uuid::now_v7(),
            tenant_id.as_uuid(),
            area_id,
            "personal",
            Some(actor_id),
            "renewal security private note",
        ),
        (
            Uuid::now_v7(),
            Uuid::now_v7(),
            tenant_id.as_uuid(),
            area_id,
            "personal",
            Some(other_actor_id),
            "renewal security other user note",
        ),
        (
            Uuid::now_v7(),
            Uuid::now_v7(),
            other_tenant_id.as_uuid(),
            other_area_id,
            "area",
            None,
            "renewal security other tenant",
        ),
    ] {
        sqlx::query("INSERT INTO memories (memory_id, tenant_id, area_id, state, origin, version) VALUES ($1, $2, $3, 'active', 'approved', 1)")
            .bind(memory_id).bind(tenant).bind(area).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO memory_versions (memory_version_id, tenant_id, memory_id, version_number, state, claim, scope, applies_when, reason, evidence, claim_hash, owner_actor_id) VALUES ($1, $2, $3, 1, 'current', $4, $5, 'always', 'live search', '[]'::jsonb, $6, $7)")
            .bind(version_id).bind(tenant).bind(memory_id).bind(claim).bind(scope).bind(format!("hash-{}", version_id)).bind(owner)
            .execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE memories SET current_version_id = $2 WHERE memory_id = $1")
            .bind(memory_id)
            .bind(version_id)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();

    let request = SearchRequest {
        authorization: AuthorizationContext {
            tenant_id,
            actor_id: Some(actor_id),
            permitted_area_ids: BTreeSet::from([area_id.into()]),
            role: ActorRole::NormalUser,
            purpose: "always".into(),
        },
        query: "renewal security".into(),
        now: OffsetDateTime::now_utc(),
        token_budget: 200,
        entry_limit: 30,
    };
    let hits = repository.search_lexical(&request).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|hit| hit.record.tenant_id == tenant_id));
    assert!(hits.iter().any(
        |hit| hit.record.visibility == engrave_core::Visibility::Private
            && hit.record.owner_actor_id == Some(actor_id)
    ));
    assert!(hits
        .iter()
        .any(|hit| hit.record.visibility == engrave_core::Visibility::Area));
    assert!(hits
        .iter()
        .all(|hit| !hit.record.claim.contains("other user")));
}
