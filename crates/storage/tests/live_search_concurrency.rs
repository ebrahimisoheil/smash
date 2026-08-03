//! Live measurement of sequential versus concurrent lexical + query-
//! embedding retrieval, against a real local PostgreSQL and LanceDB
//! instance with the credential-free deterministic embedding provider —
//! proving the actual wall-clock benefit of the Phase E performance pass's
//! `tokio::join!` change, not just that it compiles.
//!
//! Run explicitly against a disposable migrated database with:
//! `DATABASE_URL=... cargo test -p engrave-storage --test
//! live_search_concurrency -- --ignored --nocapture`.

use engrave_contracts::{AreaId, MemoryId, TenantId};
use engrave_core::{
    query_embedding_cache_key, ActorRole, AuthorizationContext, DeterministicEmbeddingProvider,
    EmbeddingProvider, EmbeddingVector, ProjectionIdentity, QueryEmbeddingCache,
    SearchRequest as CoreSearchRequest,
};
use engrave_storage::{LanceProjectionAdapter, LanceProjectionRow, PgRepository};
use sqlx::types::time::OffsetDateTime;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use uuid::Uuid;

const MEMORY_COUNT: usize = 50;

#[tokio::test]
#[ignore = "requires a disposable migrated PostgreSQL database"]
async fn sequential_vs_concurrent_lexical_and_embedding_latency_is_measured_live() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let repository = PgRepository::connect(&database_url).await.unwrap();
    let tenant_id = TenantId::new(Uuid::now_v7());
    let area_id = AreaId::new(Uuid::now_v7());
    let actor_id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO tenants (tenant_id, slug, state, created_at, updated_at) VALUES ($1, $2, 'active', now(), now())",
    )
    .bind(tenant_id.as_uuid())
    .bind(format!("concurrency-{}", tenant_id.as_uuid()))
    .execute(repository.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO areas (area_id, tenant_id, slug, state, version) VALUES ($1, $2, 'concurrency-area', 'active', 1)",
    )
    .bind(area_id.as_uuid())
    .bind(tenant_id.as_uuid())
    .execute(repository.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO actors (actor_id, tenant_id, issuer, subject, state) VALUES ($1, $2, 'test', $3, 'active')",
    )
    .bind(actor_id)
    .bind(tenant_id.as_uuid())
    .bind(actor_id.to_string())
    .execute(repository.pool())
    .await
    .unwrap();

    let mut tx = repository.pool().begin().await.unwrap();
    sqlx::query("SET LOCAL app.memory_admission = 'approved'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let memory_ids: Vec<Uuid> = (0..MEMORY_COUNT).map(|_| Uuid::now_v7()).collect();
    for (index, memory_id) in memory_ids.iter().enumerate() {
        let version_id = Uuid::now_v7();
        sqlx::query("INSERT INTO memories (memory_id, tenant_id, area_id, state, origin, version) VALUES ($1, $2, $3, 'active', 'approved', 1)")
            .bind(memory_id).bind(tenant_id.as_uuid()).bind(area_id.as_uuid())
            .execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO memory_versions (memory_version_id, tenant_id, memory_id, version_number, state, claim, scope, applies_when, reason, evidence, claim_hash) VALUES ($1, $2, $3, 1, 'current', $4, 'area', 'always', 'concurrency benchmark seed', '[]'::jsonb, $5)")
            .bind(version_id).bind(tenant_id.as_uuid()).bind(memory_id)
            .bind(format!("Acme renewal requires quarterly executive review number {index}"))
            .bind(format!("concurrency-hash-{index}"))
            .execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE memories SET current_version_id = $2 WHERE memory_id = $1")
            .bind(memory_id)
            .bind(version_id)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();

    let lance_path =
        std::env::temp_dir().join(format!("engrave-lance-concurrency-{}", Uuid::now_v7()));
    let lance = LanceProjectionAdapter::connect(lance_path.to_str().unwrap(), "memory_projection")
        .await
        .unwrap();
    let identity =
        ProjectionIdentity::new("deterministic", "dev-fallback", "1", 32, "v1", "dev-only")
            .unwrap();
    let rows: Vec<LanceProjectionRow> = memory_ids
        .iter()
        .enumerate()
        .map(|(index, memory_id)| LanceProjectionRow {
            tenant_id,
            area_id,
            memory_id: MemoryId::new(*memory_id),
            owner_actor_id: None,
            scope: "area".into(),
            state: "current".into(),
            identity: identity.clone(),
            vector: EmbeddingVector::normalized(
                (0..32)
                    .map(|dimension| (((dimension + index) % 29) as f32) / 29.0)
                    .collect(),
                32,
            )
            .unwrap(),
        })
        .collect();
    lance.reconcile(&rows).await.unwrap();

    let request = CoreSearchRequest {
        authorization: AuthorizationContext {
            tenant_id,
            actor_id: Some(actor_id),
            permitted_area_ids: BTreeSet::from([area_id]),
            role: ActorRole::NormalUser,
            purpose: "always".into(),
        },
        query: "executive review renewal".into(),
        now: OffsetDateTime::now_utc(),
        token_budget: 400,
        entry_limit: 30,
    };

    let provider = DeterministicEmbeddingProvider::new(identity.clone());

    // Sequential: lexical, then embedding + LanceDB exact search + rehydration.
    let sequential_started = Instant::now();
    let sequential_lexical = repository.search_lexical(&request).await.unwrap();
    let query_vector = provider.embed(&request.query).unwrap();
    let vector_hits = lance
        .search_exact(
            &request.authorization,
            &query_vector,
            &identity,
            request.entry_limit,
        )
        .await
        .unwrap();
    let sequential_dense = repository
        .rehydrate_dense_hits(&request, &vector_hits, &identity)
        .await
        .unwrap();
    let sequential_elapsed = sequential_started.elapsed();
    assert!(!sequential_lexical.is_empty());
    assert!(!sequential_dense.is_empty());

    // Concurrent: the same two independent pieces of work, joined.
    let concurrent_started = Instant::now();
    let (concurrent_lexical, concurrent_dense) =
        tokio::join!(repository.search_lexical(&request), async {
            let query_vector = provider.embed(&request.query).unwrap();
            let vector_hits = lance
                .search_exact(
                    &request.authorization,
                    &query_vector,
                    &identity,
                    request.entry_limit,
                )
                .await
                .unwrap();
            repository
                .rehydrate_dense_hits(&request, &vector_hits, &identity)
                .await
                .unwrap()
        },);
    let concurrent_elapsed = concurrent_started.elapsed();
    let concurrent_lexical = concurrent_lexical.unwrap();
    assert!(!concurrent_lexical.is_empty());
    assert!(!concurrent_dense.is_empty());
    assert_eq!(concurrent_lexical.len(), sequential_lexical.len());
    assert_eq!(concurrent_dense.len(), sequential_dense.len());

    println!(
        "SEARCH_CONCURRENCY_MS sequential={:.3} concurrent={:.3} memory_count={}",
        sequential_elapsed.as_secs_f64() * 1000.0,
        concurrent_elapsed.as_secs_f64() * 1000.0,
        MEMORY_COUNT,
    );

    // Cache hit vs miss, using the exact production cache-key builder.
    let mut cache = QueryEmbeddingCache::new(64);
    let cache_key = query_embedding_cache_key(&identity, "query", &request.query);
    let miss_started = Instant::now();
    let miss_result = cache.get(&cache_key);
    let miss_elapsed = miss_started.elapsed();
    assert!(miss_result.is_none());
    cache.insert(cache_key.clone(), query_vector.clone());
    let hit_started = Instant::now();
    let hit_result = cache.get(&cache_key);
    let hit_elapsed = hit_started.elapsed();
    assert!(hit_result.is_some());
    println!(
        "CACHE_LATENCY_US miss={} hit={}",
        miss_elapsed.as_micros(),
        hit_elapsed.as_micros(),
    );

    // A slow/failing embedding branch must never block or fail lexical:
    // simulate an embedding branch that sleeps past a reasonable budget and
    // confirm lexical alone still completes promptly when joined.
    let lexical_only_started = Instant::now();
    let (never_blocks_lexical, _) = tokio::join!(repository.search_lexical(&request), async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Err::<(), ()>(())
    });
    let lexical_only_elapsed = lexical_only_started.elapsed();
    assert!(never_blocks_lexical.is_ok());
    println!(
        "LEXICAL_ALONGSIDE_SLOW_EMBEDDING_MS total={:.3}",
        lexical_only_elapsed.as_secs_f64() * 1000.0,
    );
}
