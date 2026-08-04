use engrave_contracts::{AgentIdentityId, AreaId, TenantId};
use engrave_core::{AggressiveIntent, SearchBudgets, TraceState};
use engrave_storage::PgRepository;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn aggressive_trace_is_idempotent_tenant_linked_and_cancellable() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&url).await.unwrap();
    let tenant = TenantId::new_v7();
    let actor = Uuid::now_v7();
    let agent = AgentIdentityId::new_v7();
    let area = AreaId::new_v7();
    sqlx::query("INSERT INTO tenants (tenant_id,slug,state,created_at,updated_at) VALUES ($1,$2,'active',now(),now())").bind(tenant.as_uuid()).bind(format!("phase-i-{}",tenant.as_uuid())).execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO actors (actor_id,tenant_id,issuer,subject,state) VALUES ($1,$2,'test',$3,'active')").bind(actor).bind(tenant.as_uuid()).bind(actor.to_string()).execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO agent_identities (agent_identity_id,tenant_id,slug,state) VALUES ($1,$2,$3,'active')").bind(agent.as_uuid()).bind(tenant.as_uuid()).bind(agent.as_uuid().to_string()).execute(repository.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO areas (area_id,tenant_id,slug,state) VALUES ($1,$2,'investigation','active')",
    )
    .bind(area.as_uuid())
    .bind(tenant.as_uuid())
    .execute(repository.pool())
    .await
    .unwrap();
    let map = Uuid::now_v7();
    sqlx::query("INSERT INTO map_versions (map_version_id,tenant_id,area_id,version_number,state,definition,version) VALUES ($1,$2,$3,1,'draft',$4,1)")
        .bind(map).bind(tenant.as_uuid()).bind(area.as_uuid()).bind(serde_json::json!({"kinds":[],"relations":[]})).execute(repository.pool()).await.unwrap();
    let e1 = Uuid::now_v7();
    let e2 = Uuid::now_v7();
    sqlx::query("INSERT INTO entities (entity_id,tenant_id,area_id,map_version_id,kind,state,origin,descriptor,version) VALUES ($1,$2,$3,$4,'account','proposed','observed',$5,1),($6,$2,$3,$4,'account','proposed','observed',$5,1)")
        .bind(e1).bind(tenant.as_uuid()).bind(area.as_uuid()).bind(map).bind(serde_json::json!({"name":"one"})).bind(e2).execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO relationships (relationship_id,tenant_id,area_id,map_version_id,source_entity_id,target_entity_id,relation_kind,state,origin,version) VALUES ($1,$2,$3,$4,$5,$6,'related_to','proposed','observed',1)")
        .bind(Uuid::now_v7()).bind(tenant.as_uuid()).bind(area.as_uuid()).bind(map).bind(e1).bind(e2).execute(repository.pool()).await.unwrap();
    let graph = repository
        .authorized_graph_slice(tenant, &[area], 10)
        .await
        .unwrap();
    assert_eq!(graph.entities.len(), 2);
    assert_eq!(graph.relationships.len(), 1);
    let intent = AggressiveIntent {
        tenant_id: tenant,
        actor_id: actor,
        host_id: "host-i".into(),
        agent_identity_id: agent,
        session_id: Uuid::now_v7(),
        area_id: area,
        purpose: "verify".into(),
        task: "check trace".into(),
        query: "trace".into(),
        explicit: true,
        connector: None,
    };
    let budgets = SearchBudgets {
        max_steps: 2,
        max_elapsed_ms: 1000,
        max_tokens: 100,
        max_candidates: 10,
        max_external_calls: 0,
    };
    let first = repository
        .start_aggressive_search(intent.clone(), budgets.clone(), "same-key")
        .await
        .unwrap();
    let replay = repository
        .start_aggressive_search(intent, budgets, "same-key")
        .await
        .unwrap();
    assert_eq!(first.trace_id, replay.trace_id);
    assert_eq!(first.state, TraceState::Queued);
    repository
        .cancel_aggressive_search(tenant, first.trace_id.into())
        .await
        .unwrap();
    assert!(repository
        .is_cancel_requested(tenant, first.trace_id.into())
        .await
        .unwrap());
}
