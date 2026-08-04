//! Run with `DATABASE_URL=... cargo test -p engrave-mcp --test live_phase_h -- --ignored`.

use engrave_contracts::{AgentIdentityId, AreaId, MemoryId, MemoryVersionId, TenantId};
use engrave_mcp::{McpService, PgMcpBackend};
use engrave_storage::PgRepository;
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

async fn seed(
    repository: &PgRepository,
    tenant: TenantId,
    actor: Uuid,
    agents: [AgentIdentityId; 2],
    area: AreaId,
    memory: MemoryId,
    version: MemoryVersionId,
) {
    sqlx::query("INSERT INTO tenants (tenant_id,slug,state,created_at,updated_at) VALUES ($1,$2,'active',now(),now())").bind(tenant.as_uuid()).bind(format!("mcp-h-{}",tenant.as_uuid())).execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO actors (actor_id,tenant_id,issuer,subject,state) VALUES ($1,$2,'test','actor','active')").bind(actor).bind(tenant.as_uuid()).execute(repository.pool()).await.unwrap();
    let role = Uuid::now_v7();
    sqlx::query("INSERT INTO roles (role_id,tenant_id,role_key,state) VALUES ($1,$2,'normal_user','active')").bind(role).bind(tenant.as_uuid()).execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO memberships (membership_id,tenant_id,actor_id,role_id,state) VALUES ($1,$2,$3,$4,'active')").bind(Uuid::now_v7()).bind(tenant.as_uuid()).bind(actor).bind(role).execute(repository.pool()).await.unwrap();
    for agent in agents {
        sqlx::query("INSERT INTO agent_identities (agent_identity_id,tenant_id,slug,state) VALUES ($1,$2,$3,'active')").bind(agent.as_uuid()).bind(tenant.as_uuid()).bind(format!("agent-{}",agent.as_uuid())).execute(repository.pool()).await.unwrap();
    }
    sqlx::query(
        "INSERT INTO areas (area_id,tenant_id,slug,state,version) VALUES ($1,$2,'mcp','active',1)",
    )
    .bind(area.as_uuid())
    .bind(tenant.as_uuid())
    .execute(repository.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO area_grants (area_grant_id,tenant_id,area_id,actor_id,scope,state,effective_from) VALUES ($1,$2,$3,$4,'{}','active',now())").bind(Uuid::now_v7()).bind(tenant.as_uuid()).bind(area.as_uuid()).bind(actor).execute(repository.pool()).await.unwrap();
    sqlx::query("SELECT set_config('app.memory_admission','approved',false)")
        .execute(repository.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO memories (memory_id,tenant_id,area_id,state,origin,current_version_id,version) VALUES ($1,$2,$3,'active','approved',NULL,1)").bind(memory.as_uuid()).bind(tenant.as_uuid()).bind(area.as_uuid()).execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO memory_versions (memory_version_id,tenant_id,memory_id,version_number,state,claim,scope,applies_when,reason,evidence,claim_hash) VALUES ($1,$2,$3,1,'current','Reviewed UTC policy','area','always','live Phase H','[\"source-v1#chunk-1\"]','live-hash')").bind(version.as_uuid()).bind(tenant.as_uuid()).bind(memory.as_uuid()).execute(repository.pool()).await.unwrap();
    sqlx::query("UPDATE memories SET current_version_id=$2 WHERE memory_id=$1")
        .bind(memory.as_uuid())
        .bind(version.as_uuid())
        .execute(repository.pool())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn two_agent_hosts_retrieve_same_reviewed_memory_through_mcp() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repo = Arc::new(PgRepository::connect(&url).await.unwrap());
    let tenant = TenantId::new(Uuid::now_v7());
    let actor = Uuid::now_v7();
    let agents = [
        AgentIdentityId::new(Uuid::now_v7()),
        AgentIdentityId::new(Uuid::now_v7()),
    ];
    let area = AreaId::new(Uuid::now_v7());
    let memory = MemoryId::new(Uuid::now_v7());
    let version = MemoryVersionId::new(Uuid::now_v7());
    seed(&repo, tenant, actor, agents, area, memory, version).await;
    let service = McpService::new(
        Arc::new(PgMcpBackend {
            repository: repo.clone(),
        }),
        vec![],
    )
    .unwrap();
    for (id, agent) in agents.into_iter().enumerate() {
        let request = json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"recall","arguments":{"context":{"tenant_id":tenant,"actor_id":actor,"host_id":format!("host-{id}"),"agent_identity_id":agent,"session_id":Uuid::now_v7(),"purpose":"always","role":"normal_user","area_id":area},"query":"UTC"}}});
        let response = service.handle_json(&request.to_string()).await;
        assert!(response.contains("Reviewed UTC policy"), "{response}");
    }
    let proposal_request = json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"propose_memory","arguments":{"context":{"tenant_id":tenant,"actor_id":actor,"host_id":"host-session-end","agent_identity_id":agents[0],"session_id":Uuid::now_v7(),"purpose":"session_end","role":"normal_user","area_id":area},"claim":"Capture only as proposal","evidence":["source-v1#chunk-1"]}}});
    let proposal_response = service.handle_json(&proposal_request.to_string()).await;
    assert!(proposal_response.contains("pending"), "{proposal_response}");
    let pending = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM proposals WHERE tenant_id=$1 AND state='pending' AND payload->>'claim'='Capture only as proposal'").bind(tenant.as_uuid()).fetch_one(repo.pool()).await.unwrap();
    assert_eq!(pending, 1);
    let count =
        sqlx::query("SELECT count(*)::bigint AS count FROM rule_decisions WHERE tenant_id=$1")
            .bind(tenant.as_uuid())
            .fetch_one(repo.pool())
            .await
            .unwrap()
            .get::<i64, _>("count");
    assert!(count >= 2);
}

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn workspace_setup_is_durable_replay_safe_and_proposal_only() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repo = Arc::new(PgRepository::connect(&url).await.unwrap());
    let tenant = TenantId::new(Uuid::now_v7());
    let actor = Uuid::now_v7();
    let agent = AgentIdentityId::new(Uuid::now_v7());
    let area = AreaId::new(Uuid::now_v7());
    let memory = MemoryId::new(Uuid::now_v7());
    let version = MemoryVersionId::new(Uuid::now_v7());
    seed(
        &repo,
        tenant,
        actor,
        [agent, AgentIdentityId::new(Uuid::now_v7())],
        area,
        memory,
        version,
    )
    .await;
    let service = McpService::new(
        Arc::new(PgMcpBackend {
            repository: repo.clone(),
        }),
        vec![],
    )
    .unwrap();
    let session = Uuid::now_v7();
    let context = json!({
        "tenant_id": tenant,
        "actor_id": actor,
        "host_id": "workspace-host",
        "agent_identity_id": agent,
        "session_id": session,
        "purpose": "workspace_setup",
        "role": "normal_user",
        "area_id": area
    });
    let begin: serde_json::Value = serde_json::from_str(
        &service
            .handle_json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context.clone(),"action":"begin","idempotency_key":"live-workspace-1"}}}).to_string())
            .await,
    )
    .unwrap();
    let interview_id = begin["result"]["interview_id"].as_str().unwrap().to_owned();
    assert!(begin["result"]["area_options"].is_array());
    assert!(begin["result"]["request_new_area"] == json!(true));
    let narrowed = service
        .handle_json(
            &json!({"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context.clone(),"action":"begin","selected_area_ids":[Uuid::now_v7().to_string()],"idempotency_key":"live-workspace-narrowed"}}}).to_string(),
        )
        .await;
    assert!(
        narrowed.contains("Forbidden") || narrowed.contains("-32000"),
        "{narrowed}"
    );
    let replay: serde_json::Value = serde_json::from_str(
        &service.handle_json(&json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context.clone(),"action":"begin","idempotency_key":"live-workspace-1"}}}).to_string()).await,
    ).unwrap();
    assert_eq!(
        replay["result"]["interview_id"].as_str(),
        Some(interview_id.as_str())
    );
    let draft = service.handle_json(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context.clone(),"action":"draft","interview_id":interview_id.clone(),"ontology_draft":{"kinds":["customer"],"relationships":[],"assumptions":[],"unresolved_questions":[]}}}}).to_string()).await;
    assert!(draft.contains("awaiting_confirmation"), "{draft}");
    let confirmed = service.handle_json(&json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context.clone(),"action":"confirm","interview_id":interview_id.clone(),"confirmed":true}}}).to_string()).await;
    assert!(confirmed.contains("confirmed"), "{confirmed}");
    let confirm_replay = service.handle_json(&json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context.clone(),"action":"confirm","interview_id":interview_id.clone(),"confirmed":true}}}).to_string()).await;
    assert!(confirm_replay.contains("confirmed"), "{confirm_replay}");
    let submitted = service.handle_json(&json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context.clone(),"action":"submit","interview_id":interview_id}}}).to_string()).await;
    assert!(submitted.contains("submitted"), "{submitted}");
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM proposals WHERE tenant_id=$1 AND kind='map_area' AND state='pending'",
    )
    .bind(tenant.as_uuid())
    .fetch_one(repo.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
    let memories = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memories WHERE tenant_id=$1")
        .bind(tenant.as_uuid())
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(memories, 1);

    let cancellable: serde_json::Value = serde_json::from_str(
        &service
            .handle_json(&json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context.clone(),"action":"begin","requested_areas":[{"name":"Requested review area"}],"idempotency_key":"live-workspace-cancel"}}}).to_string())
            .await,
    )
    .unwrap();
    let cancel_id = cancellable["result"]["interview_id"].as_str().unwrap();
    assert_eq!(
        cancellable["result"]["requested_areas"][0]["name"],
        json!("Requested review area")
    );
    let cancelled = service
        .handle_json(&json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context.clone(),"action":"cancel","interview_id":cancel_id}}}).to_string())
        .await;
    assert!(cancelled.contains("cancelled"), "{cancelled}");
    let cancel_replay = service
        .handle_json(&json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"workspace_setup","arguments":{"context":context,"action":"cancel","interview_id":cancel_id}}}).to_string())
        .await;
    assert!(cancel_replay.contains("cancelled"), "{cancel_replay}");
}
