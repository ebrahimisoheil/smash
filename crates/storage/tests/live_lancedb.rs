use engrave_contracts::{AreaId, MemoryId, TenantId};
use engrave_core::{ActorRole, AuthorizationContext, EmbeddingVector, ProjectionIdentity};
use engrave_storage::{LanceProjectionAdapter, LanceProjectionRow};
use std::collections::BTreeSet;
use std::time::Instant;
use uuid::Uuid;

#[tokio::test]
async fn lancedb_projection_filters_tenant_area_and_private_owner() {
    let path = std::env::temp_dir().join(format!("engrave-lance-{}", Uuid::now_v7()));
    let adapter = LanceProjectionAdapter::connect(path.to_str().unwrap(), "memory_projection")
        .await
        .unwrap();
    let tenant = TenantId::new_v7();
    let other_tenant = TenantId::new_v7();
    let area = AreaId::new_v7();
    let other_area = AreaId::new_v7();
    let actor = Uuid::now_v7();
    let other_actor = Uuid::now_v7();
    let identity = ProjectionIdentity::new("test", "unit", "1", 2, "p1", "fp1").unwrap();
    let vector = |values| EmbeddingVector::normalized(values, 2).unwrap();
    let row =
        |tenant: TenantId, area: AreaId, scope: &str, owner: Option<Uuid>, values: Vec<f32>| {
            LanceProjectionRow {
                tenant_id: tenant,
                area_id: area,
                memory_id: MemoryId::new_v7(),
                owner_actor_id: owner,
                scope: scope.into(),
                state: "current".into(),
                identity: identity.clone(),
                vector: vector(values),
            }
        };
    let rows = vec![
        row(tenant, area, "area", None, vec![1.0, 0.0]),
        row(tenant, area, "personal", Some(actor), vec![0.9, 0.1]),
        row(tenant, area, "personal", Some(other_actor), vec![0.8, 0.2]),
        row(tenant, other_area, "area", None, vec![0.7, 0.3]),
        row(other_tenant, area, "area", None, vec![0.6, 0.4]),
    ];
    adapter.reconcile(&rows).await.unwrap();

    let authorization = AuthorizationContext {
        tenant_id: tenant,
        actor_id: Some(actor),
        permitted_area_ids: BTreeSet::from([area]),
        role: ActorRole::NormalUser,
        purpose: "always".into(),
    };
    let hits = adapter
        .search_exact(&authorization, &vector(vec![1.0, 0.0]), &identity, 30)
        .await
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits
        .iter()
        .all(|hit| hit.tenant_id == tenant && hit.area_id == area));
    assert!(hits.iter().any(|hit| hit.owner_actor_id == Some(actor)));
    assert!(hits
        .iter()
        .all(|hit| hit.owner_actor_id != Some(other_actor)));

    let mut latencies = Vec::new();
    for _ in 0..100 {
        let started = Instant::now();
        adapter
            .search_exact(&authorization, &vector(vec![1.0, 0.0]), &identity, 30)
            .await
            .unwrap();
        latencies.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    latencies.sort_by(f64::total_cmp);
    let percentile = |fraction: f64| latencies[((latencies.len() - 1) as f64 * fraction) as usize];
    println!(
        "LANCEDB_EXACT_LATENCY_MS p50={:.3} p95={:.3} p99={:.3}",
        percentile(0.50),
        percentile(0.95),
        percentile(0.99)
    );
}

#[tokio::test]
async fn lancedb_exact_latency_on_1000_production_dimension_rows() {
    let path = std::env::temp_dir().join(format!("engrave-lance-capacity-{}", Uuid::now_v7()));
    let adapter = LanceProjectionAdapter::connect(path.to_str().unwrap(), "memory_projection")
        .await
        .unwrap();
    let tenant = TenantId::new_v7();
    let area = AreaId::new_v7();
    let actor = Uuid::now_v7();
    let identity = ProjectionIdentity::new(
        "capacity-test",
        "production-dimension",
        "1",
        1024,
        "p1",
        "capacity-fp",
    )
    .unwrap();
    let vector = |seed: usize| {
        let values = (0..1024)
            .map(|index| (((index + seed) % 29) as f32) / 29.0)
            .collect::<Vec<_>>();
        EmbeddingVector::normalized(values, 1024).unwrap()
    };
    let rows = (0..1000)
        .map(|index| LanceProjectionRow {
            tenant_id: tenant,
            area_id: area,
            memory_id: MemoryId::new_v7(),
            owner_actor_id: if index % 11 == 0 { Some(actor) } else { None },
            scope: if index % 11 == 0 { "personal" } else { "area" }.into(),
            state: "current".into(),
            identity: identity.clone(),
            vector: vector(index),
        })
        .collect::<Vec<_>>();
    adapter.reconcile(&rows).await.unwrap();
    adapter.build_ann_index().await.unwrap();
    let authorization = AuthorizationContext {
        tenant_id: tenant,
        actor_id: Some(actor),
        permitted_area_ids: BTreeSet::from([area]),
        role: ActorRole::NormalUser,
        purpose: "always".into(),
    };
    let query = vector(7);
    let mut latencies = Vec::new();
    for _ in 0..100 {
        let started = Instant::now();
        let hits = adapter
            .search_exact(&authorization, &query, &identity, 10)
            .await
            .unwrap();
        assert!(!hits.is_empty());
        latencies.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    latencies.sort_by(f64::total_cmp);
    let percentile = |fraction: f64| latencies[((latencies.len() - 1) as f64 * fraction) as usize];
    println!(
        "LANCEDB_EXACT_CAPACITY_1000_DIM1024_MS p50={:.3} p95={:.3} p99={:.3}",
        percentile(0.50),
        percentile(0.95),
        percentile(0.99)
    );

    let mut ann_latencies = Vec::new();
    for _ in 0..100 {
        let started = Instant::now();
        let hits = adapter
            .search_ann(&authorization, &query, &identity, 10)
            .await
            .unwrap();
        assert!(!hits.is_empty());
        ann_latencies.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    ann_latencies.sort_by(f64::total_cmp);
    let ann_percentile =
        |fraction: f64| ann_latencies[((ann_latencies.len() - 1) as f64 * fraction) as usize];
    println!(
        "LANCEDB_ANN_CAPACITY_1000_DIM1024_MS p50={:.3} p95={:.3} p99={:.3}",
        ann_percentile(0.50),
        ann_percentile(0.95),
        ann_percentile(0.99)
    );
}
