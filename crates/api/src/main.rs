use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use engrave_contracts::{
    AreaId, CrossMapMapping, CrossMapMappingId, Entity, EntityId, MapDefinition, MapVersionId,
    ProposalId, Relationship, RelationshipId, SourceState,
};
use engrave_core::{
    bounded_traverse, light_search, ApplicationError, CircuitBreaker, CrossMapProposalInput,
    CrossMapReviewAction, CrossMapReviewError, CrossMapStore, DegradedMode, DenseHit,
    DeterministicEmbeddingProvider, EmbeddingConfiguration, EmbeddingProvider, EmbeddingVector,
    EntityDraftInput, EntityReviewAction, EntityReviewError, EntityStore, GraphBudget,
    MapDraftInput, MapPublicationPolicy, MapReviewAction, MapReviewError, MapStore, MemoryStore,
    ProjectionIdentity, ProposalInput, QueryEmbeddingCache, RelationshipDraftInput,
    RelationshipReviewAction, ReviewAction, SearchProfile, SearchRequest as CoreSearchRequest,
};
use engrave_storage::{
    LanceProjectionAdapter, OpenAiEmbeddingClient, PgRepository, VoyageEmbeddingClient,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;
use utoipa::{OpenApi, ToSchema};

#[derive(Debug, Serialize, ToSchema)]
struct HealthResponse {
    status: &'static str,
    readiness: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
struct VersionResponse {
    service: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
struct ProcessingState {
    state: SourceState,
    terminal: bool,
    actionable: bool,
}

#[derive(Clone)]
struct AppState {
    memories: Arc<Mutex<MemoryStore>>,
    maps: Arc<Mutex<MapStore>>,
    entities: Arc<Mutex<EntityStore>>,
    cross_map: Arc<Mutex<CrossMapStore>>,
    repository: Option<Arc<PgRepository>>,
    lance: Option<Arc<LanceProjectionAdapter>>,
    embedding_cache: Arc<Mutex<QueryEmbeddingCache>>,
    embedding_circuit: Arc<Mutex<CircuitBreaker>>,
    embedding_concurrency: Arc<tokio::sync::Semaphore>,
}

#[derive(Debug, Deserialize, ToSchema)]
struct SearchRequest {
    query: String,
    #[serde(default = "default_token_budget")]
    token_budget: usize,
    #[serde(default = "default_entry_limit")]
    entry_limit: usize,
}

fn default_token_budget() -> usize {
    400
}
fn default_entry_limit() -> usize {
    10
}

#[derive(Debug, Serialize, ToSchema)]
struct SearchResult {
    memory_id: String,
    area_id: String,
    claim: String,
    reason: String,
    provenance: Vec<String>,
    applicability: String,
    warnings: Vec<String>,
    estimated_tokens: usize,
}

#[derive(Debug, Serialize, ToSchema)]
struct SearchResponse {
    results: Vec<SearchResult>,
    tokens_used: usize,
    lexical_candidates: usize,
    dense_candidates: usize,
    degraded_mode: String,
}

#[derive(Debug, Deserialize)]
struct ReviewRequest {
    expected_version: u64,
    idempotency_key: String,
    action: ReviewAction,
}

fn actor(headers: &HeaderMap) -> Result<String, axum::http::StatusCode> {
    headers
        .get("x-actor-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)
}

async fn create_proposal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut input): Json<ProposalInput>,
) -> Result<Json<engrave_core::memory::Proposal>, axum::http::StatusCode> {
    let caller = actor(&headers)?;
    input.proposer = caller;
    let mut store = state
        .memories
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(store.propose(ProposalId::new_v7(), input)))
}

async fn review_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ReviewRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let reviewer = actor(&headers)?;
    let proposal_id = uuid::Uuid::parse_str(&id)
        .map(ProposalId::new)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let mut store = state
        .memories
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let memory_id = store
        .review(
            proposal_id,
            &reviewer,
            request.expected_version,
            &request.idempotency_key,
            request.action,
        )
        .map_err(|error| match error {
            engrave_core::ReviewError::VersionConflict { .. } => axum::http::StatusCode::CONFLICT,
            engrave_core::ReviewError::IndependentReviewRequired => {
                axum::http::StatusCode::FORBIDDEN
            }
            engrave_core::ReviewError::NotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        })?;
    Ok(Json(
        serde_json::json!({"state":"accepted_or_updated","memory_id":memory_id,"explainability":"Activity is appended by the application service"}),
    ))
}

// --- Phase F: Map, Entity/Relationship, Graph, and Cross-Map review ---
//
// These routes mirror the Memory proposal routes above: an in-memory
// reference store behind `AppState`, gated only by the `x-actor-id` header.
// Like `/v1/memory/proposals`, they are a functional reference surface, not
// yet the fully authorization-enforced live path that `/v1/search` has (that
// requires a PostgreSQL-backed Area-grant resolver, out of scope for this
// session). They are intentionally not added to `ApiDoc`'s `paths(...)`
// list, matching how the existing Memory proposal routes are already
// callable without being part of the published OpenAPI contract.

#[derive(Debug, Deserialize)]
struct MapDraftRequest {
    tenant_id: uuid::Uuid,
    area_id: uuid::Uuid,
    reason: String,
    definition: MapDefinition,
    policy: MapPublicationPolicy,
    predecessor: Option<uuid::Uuid>,
}

#[derive(Debug, Deserialize)]
struct MapReviewRequest {
    expected_version: u64,
    idempotency_key: String,
    action: MapReviewAction,
}

fn map_error_status(error: MapReviewError) -> axum::http::StatusCode {
    match error {
        MapReviewError::NotFound => axum::http::StatusCode::NOT_FOUND,
        MapReviewError::VersionConflict { .. } => axum::http::StatusCode::CONFLICT,
        MapReviewError::IndependentReviewRequired => axum::http::StatusCode::FORBIDDEN,
        MapReviewError::InvalidState | MapReviewError::EmptyDefinition => {
            axum::http::StatusCode::UNPROCESSABLE_ENTITY
        }
    }
}

async fn propose_map_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<MapDraftRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let proposer = actor(&headers)?;
    let draft_input = MapDraftInput {
        tenant_id: input.tenant_id.into(),
        area_id: input.area_id.into(),
        proposer,
        reason: input.reason,
        definition: input.definition,
        policy: input.policy,
        predecessor: input.predecessor.map(MapVersionId::new),
    };
    let mut store = state
        .maps
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let draft = store
        .propose_draft(MapVersionId::new_v7(), draft_input)
        .map_err(map_error_status)?;
    Ok(Json(serde_json::json!({
        "map_version_id": draft.id.as_uuid().to_string(),
        "version_number": draft.version_number,
        "state": format!("{:?}", draft.state),
    })))
}

async fn review_map_draft(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<MapReviewRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let reviewer = actor(&headers)?;
    let map_version_id = uuid::Uuid::parse_str(&id)
        .map(MapVersionId::new)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let mut store = state
        .maps
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let published = store
        .review(
            map_version_id,
            &reviewer,
            request.expected_version,
            &request.idempotency_key,
            request.action,
        )
        .map_err(map_error_status)?;
    Ok(Json(serde_json::json!({
        "map_version_id": map_version_id.as_uuid().to_string(),
        "published": published.is_some(),
    })))
}

async fn propose_entity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut input): Json<EntityDraftInput>,
) -> Result<Json<Entity>, axum::http::StatusCode> {
    input.proposer = actor(&headers)?;
    let mut store = state
        .entities
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let entity = store
        .propose_entity(EntityId::new_v7(), input)
        .map_err(entity_error_status)?;
    Ok(Json(entity))
}

#[derive(Debug, Deserialize)]
struct EntityReviewRequest {
    expected_version: u64,
    idempotency_key: String,
    action: EntityReviewAction,
}

fn entity_error_status(error: EntityReviewError) -> axum::http::StatusCode {
    match error {
        EntityReviewError::NotFound => axum::http::StatusCode::NOT_FOUND,
        EntityReviewError::VersionConflict { .. } => axum::http::StatusCode::CONFLICT,
        EntityReviewError::IndependentReviewRequired => axum::http::StatusCode::FORBIDDEN,
        EntityReviewError::InvalidState
        | EntityReviewError::UnknownKind
        | EntityReviewError::UnknownRelation
        | EntityReviewError::DanglingEntityReference
        | EntityReviewError::KindMismatch => axum::http::StatusCode::UNPROCESSABLE_ENTITY,
    }
}

async fn review_entity(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<EntityReviewRequest>,
) -> Result<Json<Entity>, axum::http::StatusCode> {
    let reviewer = actor(&headers)?;
    let entity_id = uuid::Uuid::parse_str(&id)
        .map(EntityId::new)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let mut store = state
        .entities
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let entity = store
        .review_entity(
            entity_id,
            &reviewer,
            request.expected_version,
            &request.idempotency_key,
            request.action,
        )
        .map_err(entity_error_status)?;
    Ok(Json(entity))
}

async fn propose_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut input): Json<RelationshipDraftInput>,
) -> Result<Json<Relationship>, axum::http::StatusCode> {
    input.proposer = actor(&headers)?;
    let mut store = state
        .entities
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let relationship = store
        .propose_relationship(RelationshipId::new_v7(), input)
        .map_err(entity_error_status)?;
    Ok(Json(relationship))
}

#[derive(Debug, Deserialize)]
struct RelationshipReviewRequest {
    expected_version: u64,
    idempotency_key: String,
    action: RelationshipReviewAction,
}

async fn review_relationship(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RelationshipReviewRequest>,
) -> Result<Json<Relationship>, axum::http::StatusCode> {
    let reviewer = actor(&headers)?;
    let relationship_id = uuid::Uuid::parse_str(&id)
        .map(RelationshipId::new)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let mut store = state
        .entities
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let relationship = store
        .review_relationship(
            relationship_id,
            &reviewer,
            request.expected_version,
            &request.idempotency_key,
            request.action,
        )
        .map_err(entity_error_status)?;
    Ok(Json(relationship))
}

/// Board review honesty surface: which Area-local Entities currently resolve
/// to the same identity, without ever hiding or deleting a member.
async fn identity_groups(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, axum::http::StatusCode> {
    let store = state
        .entities
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        store
            .identity_groups()
            .into_iter()
            .map(|group| {
                serde_json::json!({
                    "canonical": group.canonical.as_uuid().to_string(),
                    "members": group.members.iter().map(|id| id.as_uuid().to_string()).collect::<Vec<_>>(),
                })
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
struct GraphQueryRequest {
    area_id: uuid::Uuid,
    start: Vec<uuid::Uuid>,
    #[serde(default)]
    max_depth: Option<u32>,
    #[serde(default)]
    max_nodes: Option<usize>,
    #[serde(default)]
    max_edges: Option<usize>,
}

/// Bounded graph view: reads only the requested Area's own Entities and
/// Relationships from the in-memory reference store (no Cross-Map crossing —
/// that is `expand_cross_map_candidates`'s job, invoked separately) and
/// walks outward from `start` within an explicit, never-unbounded budget.
async fn graph_query(
    State(state): State<AppState>,
    Json(request): Json<GraphQueryRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let area_id: AreaId = request.area_id.into();
    let store = state
        .entities
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let entities = store.entities_in_area(area_id);
    let relationships = store.relationships_in_area(area_id);
    drop(store);

    let default_budget = GraphBudget::default();
    let budget = GraphBudget {
        max_depth: request.max_depth.unwrap_or(default_budget.max_depth),
        max_nodes: request.max_nodes.unwrap_or(default_budget.max_nodes),
        max_edges: request.max_edges.unwrap_or(default_budget.max_edges),
    };
    let start: Vec<EntityId> = request.start.into_iter().map(EntityId::new).collect();
    let packet = bounded_traverse(&start, &entities, &relationships, budget);
    Ok(Json(serde_json::json!({
        "nodes": packet.nodes.iter().map(|node| serde_json::json!({
            "entity_id": node.entity_id.as_uuid().to_string(),
            "depth": node.depth,
        })).collect::<Vec<_>>(),
        "edges": packet.edges.iter().map(|edge| serde_json::json!({
            "relationship_id": edge.relationship_id.as_uuid().to_string(),
            "source_entity_id": edge.source_entity_id.as_uuid().to_string(),
            "target_entity_id": edge.target_entity_id.as_uuid().to_string(),
            "relation_kind": edge.relation_kind,
        })).collect::<Vec<_>>(),
        "truncated": packet.truncated,
    })))
}

async fn propose_cross_map_mapping(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut input): Json<CrossMapProposalInput>,
) -> Result<Json<CrossMapMapping>, axum::http::StatusCode> {
    input.proposer = actor(&headers)?;
    let mut store = state
        .cross_map
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(store.propose(CrossMapMappingId::new_v7(), input)))
}

#[derive(Debug, Deserialize)]
struct CrossMapReviewRequest {
    expected_version: u64,
    idempotency_key: String,
    action: CrossMapReviewAction,
}

fn cross_map_error_status(error: CrossMapReviewError) -> axum::http::StatusCode {
    match error {
        CrossMapReviewError::NotFound => axum::http::StatusCode::NOT_FOUND,
        CrossMapReviewError::VersionConflict { .. } => axum::http::StatusCode::CONFLICT,
        CrossMapReviewError::IndependentReviewRequired => axum::http::StatusCode::FORBIDDEN,
        CrossMapReviewError::InvalidState => axum::http::StatusCode::UNPROCESSABLE_ENTITY,
    }
}

async fn review_cross_map_mapping(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CrossMapReviewRequest>,
) -> Result<Json<CrossMapMapping>, axum::http::StatusCode> {
    let reviewer = actor(&headers)?;
    let mapping_id = uuid::Uuid::parse_str(&id)
        .map(CrossMapMappingId::new)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let mut store = state
        .cross_map
        .lock()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let mapping = store
        .review(
            mapping_id,
            &reviewer,
            request.expected_version,
            &request.idempotency_key,
            request.action,
        )
        .map_err(cross_map_error_status)?;
    Ok(Json(mapping))
}

fn retrieval_headers(
    headers: &HeaderMap,
) -> Result<(uuid::Uuid, uuid::Uuid, Vec<uuid::Uuid>, String), axum::http::StatusCode> {
    let tenant_id = headers
        .get("x-tenant-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
    let actor_id = headers
        .get("x-actor-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
    let areas = headers
        .get("x-area-ids")
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .filter_map(|area| uuid::Uuid::parse_str(area.trim()).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let purpose = headers
        .get("x-search-purpose")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("always")
        .to_owned();
    Ok((tenant_id, actor_id, areas, purpose))
}

async fn dense_hits(
    repository: &PgRepository,
    lance: &LanceProjectionAdapter,
    request: &CoreSearchRequest,
    query_vector: &EmbeddingVector,
    identity: &ProjectionIdentity,
) -> Result<Vec<DenseHit>, ApplicationError> {
    let vector_hits = if std::env::var("ENGRAVE_RETRIEVAL_SEARCH").as_deref() == Ok("ann") {
        lance
            .search_ann(
                &request.authorization,
                query_vector,
                identity,
                request.entry_limit,
            )
            .await
    } else {
        lance
            .search_exact(
                &request.authorization,
                query_vector,
                identity,
                request.entry_limit,
            )
            .await
    }
    .map_err(|_| ApplicationError::DependencyUnavailable {
        dependency: "lancedb",
    })?;
    repository
        .rehydrate_dense_hits(request, &vector_hits, identity)
        .await
}

fn record_provider_success(state: &AppState) {
    if let Ok(mut breaker) = state.embedding_circuit.lock() {
        breaker.record_success();
    }
}

fn record_provider_failure(state: &AppState) {
    if let Ok(mut breaker) = state.embedding_circuit.lock() {
        let now_ms = (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as u64;
        breaker.record_failure(now_ms);
    }
}

#[utoipa::path(
    post,
    path = "/v1/search",
    request_body = SearchRequest,
    responses((status = 200, body = SearchResponse))
)]
async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, axum::http::StatusCode> {
    let (tenant_id, actor_id, requested_areas, purpose) = retrieval_headers(&headers)?;
    let repository = state
        .repository
        .clone()
        .ok_or(axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
    let authorization = repository
        .resolve_search_authorization(tenant_id.into(), actor_id, &requested_areas, purpose)
        .await
        .map_err(|error| match error {
            engrave_core::ApplicationError::Forbidden => axum::http::StatusCode::FORBIDDEN,
            _ => axum::http::StatusCode::SERVICE_UNAVAILABLE,
        })?;
    let request = CoreSearchRequest {
        authorization,
        query: input.query,
        now: OffsetDateTime::now_utc(),
        token_budget: input.token_budget,
        entry_limit: input.entry_limit,
    };
    let lexical = repository
        .search_lexical(&request)
        .await
        .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
    let mut dense = Vec::new();
    let mut degraded = DegradedMode::LexicalOnly {
        reason: "no embedding provider configured; lexical fallback".into(),
    };
    let provider_allowed = state
        .embedding_circuit
        .lock()
        .map(|mut breaker| {
            let now_ms = (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as u64;
            breaker.allow_request(now_ms)
        })
        .unwrap_or(false);
    let embedding_permit = tokio::time::timeout(
        std::time::Duration::from_millis(250),
        state.embedding_concurrency.clone().acquire_owned(),
    )
    .await
    .ok()
    .and_then(Result::ok);
    let deterministic_profile =
        std::env::var("ENGRAVE_EMBEDDING_PROFILE").ok().as_deref() == Some("deterministic-dev");
    if embedding_permit.is_none() {
        degraded = DegradedMode::LexicalOnly {
            reason: "embedding concurrency bound timed out; lexical fallback".into(),
        };
    } else if !deterministic_profile && !provider_allowed {
        degraded = DegradedMode::LexicalOnly {
            reason: "embedding provider circuit is open; lexical fallback".into(),
        };
    }
    if let (Some(lance), Ok(profile_name), true) = (
        state.lance.clone(),
        std::env::var("ENGRAVE_EMBEDDING_PROFILE"),
        (deterministic_profile || provider_allowed) && embedding_permit.is_some(),
    ) {
        let identity = if profile_name == "deterministic-dev" {
            ProjectionIdentity::new("deterministic", "dev-fallback", "1", 32, "v1", "dev-only")
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        } else {
            EmbeddingConfiguration::production_candidates()
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
                .profile(&profile_name)
                .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?
                .identity()
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        };
        let cache_key = format!(
            "{}:{}:{}:{}:{}",
            profile_name,
            identity.model,
            identity.model_version,
            identity.configuration_fingerprint,
            request.query
        );
        let cached_query = state
            .embedding_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&cache_key));
        if let Some(query_vector) = cached_query {
            if let Ok(rehydrated) =
                dense_hits(&repository, &lance, &request, &query_vector, &identity).await
            {
                dense = rehydrated;
                degraded = DegradedMode::None;
            }
        } else if profile_name == "deterministic-dev" {
            let provider = DeterministicEmbeddingProvider::new(identity.clone());
            if let Ok(query_vector) = provider.embed(&request.query) {
                if let Ok(mut cache) = state.embedding_cache.lock() {
                    cache.insert(cache_key.clone(), query_vector.clone());
                }
                if let Ok(rehydrated) =
                    dense_hits(&repository, &lance, &request, &query_vector, &identity).await
                {
                    dense = rehydrated;
                    degraded = DegradedMode::None;
                }
            }
        } else if profile_name == "voyage-3-lite" {
            match VoyageEmbeddingClient::from_env() {
                Ok(client) => match client.embed_projected(&request.query, &identity).await {
                    Ok(values) => match EmbeddingVector::normalized(values, identity.dimension) {
                        Ok(query_vector) => {
                            record_provider_success(&state);
                            if let Ok(mut cache) = state.embedding_cache.lock() {
                                cache.insert(cache_key.clone(), query_vector.clone());
                            }
                            if let Ok(rehydrated) =
                                dense_hits(&repository, &lance, &request, &query_vector, &identity)
                                    .await
                            {
                                dense = rehydrated;
                                degraded = DegradedMode::None;
                            }
                        }
                        Err(_) => {
                            record_provider_failure(&state);
                            degraded = DegradedMode::LexicalOnly {
                                reason: "Voyage projection produced an invalid vector".into(),
                            }
                        }
                    },
                    Err(error) => {
                        record_provider_failure(&state);
                        degraded = DegradedMode::LexicalOnly {
                            reason: format!("Voyage provider degraded: {error:?}"),
                        }
                    }
                },
                Err(error) => {
                    record_provider_failure(&state);
                    degraded = DegradedMode::LexicalOnly {
                        reason: format!("Voyage provider configuration unavailable: {error:?}"),
                    }
                }
            }
        } else if profile_name == "openai-large" {
            match OpenAiEmbeddingClient::from_env() {
                Ok(client) => match client.embed_projected(&request.query, &identity).await {
                    Ok(values) => match EmbeddingVector::normalized(values, identity.dimension) {
                        Ok(query_vector) => {
                            record_provider_success(&state);
                            if let Ok(mut cache) = state.embedding_cache.lock() {
                                cache.insert(cache_key.clone(), query_vector.clone());
                            }
                            if let Ok(rehydrated) =
                                dense_hits(&repository, &lance, &request, &query_vector, &identity)
                                    .await
                            {
                                dense = rehydrated;
                                degraded = DegradedMode::None;
                            }
                        }
                        Err(_) => {
                            record_provider_failure(&state);
                            degraded = DegradedMode::LexicalOnly {
                                reason: "OpenAI projection produced an invalid vector".into(),
                            }
                        }
                    },
                    Err(error) => {
                        record_provider_failure(&state);
                        degraded = DegradedMode::LexicalOnly {
                            reason: format!("OpenAI provider degraded: {error:?}"),
                        }
                    }
                },
                Err(error) => {
                    record_provider_failure(&state);
                    degraded = DegradedMode::LexicalOnly {
                        reason: format!("OpenAI provider configuration unavailable: {error:?}"),
                    }
                }
            }
        } else {
            degraded = DegradedMode::LexicalOnly {
                reason: format!(
                    "provider profile '{profile_name}' requires its configured provider adapter"
                ),
            };
        }
    }
    let packet = light_search(
        &request,
        &lexical,
        &dense,
        &SearchProfile::default(),
        degraded,
    );
    Ok(Json(SearchResponse {
        results: packet
            .results
            .into_iter()
            .map(|result| SearchResult {
                memory_id: result.memory_id.as_uuid().to_string(),
                area_id: result.area_id.as_uuid().to_string(),
                claim: result.claim,
                reason: result.reason,
                provenance: result.provenance,
                applicability: result.applicability,
                warnings: result.warnings,
                estimated_tokens: result.estimated_tokens,
            })
            .collect(),
        tokens_used: packet.tokens_used,
        lexical_candidates: packet.trace.lexical_candidates,
        dense_candidates: packet.trace.dense_candidates,
        degraded_mode: format!("{:?}", packet.trace.degraded_mode),
    }))
}

#[utoipa::path(get, path = "/v1/processing-states", responses((status = 200, body = [ProcessingState])))]
async fn processing_states() -> Json<Vec<ProcessingState>> {
    Json(
        vec![
            (SourceState::Uploaded, false, true),
            (SourceState::Verified, false, true),
            (SourceState::Queued, false, true),
            (SourceState::Extracting, false, true),
            (SourceState::Chunking, false, true),
            (SourceState::Indexing, false, true),
            (SourceState::Proposing, false, true),
            (SourceState::Ready, true, false),
            (SourceState::PartiallyReady, true, true),
            (SourceState::Failed, true, true),
            (SourceState::Quarantined, true, true),
            (SourceState::Deleted, true, false),
        ]
        .into_iter()
        .map(|(state, terminal, actionable)| ProcessingState {
            state,
            terminal,
            actionable,
        })
        .collect(),
    )
}

#[utoipa::path(get, path = "/v1/health", responses((status = 200, body = HealthResponse)))]
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        readiness: "not_configured",
    })
}

#[utoipa::path(get, path = "/v1/version", responses((status = 200, body = VersionResponse)))]
async fn version() -> Json<VersionResponse> {
    Json(VersionResponse {
        service: "engrave-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(OpenApi)]
#[openapi(
    paths(health, version, processing_states, search),
    components(schemas(HealthResponse, VersionResponse, ProcessingState, SearchRequest, SearchResult, SearchResponse, SourceState)),
    info(title = "ENGRAVE V2 API", version = env!("CARGO_PKG_VERSION"))
)]
struct ApiDoc;

#[cfg(test)]
fn app() -> Router {
    app_with_retrieval(None, None)
}

fn app_with_retrieval(
    repository: Option<Arc<PgRepository>>,
    lance: Option<Arc<LanceProjectionAdapter>>,
) -> Router {
    let state = AppState {
        memories: Arc::new(Mutex::new(MemoryStore::default())),
        maps: Arc::new(Mutex::new(MapStore::default())),
        entities: Arc::new(Mutex::new(EntityStore::default())),
        cross_map: Arc::new(Mutex::new(CrossMapStore::default())),
        repository,
        lance,
        embedding_cache: Arc::new(Mutex::new(QueryEmbeddingCache::new(512))),
        embedding_circuit: Arc::new(Mutex::new(CircuitBreaker::new(3, 1_000))),
        embedding_concurrency: Arc::new(tokio::sync::Semaphore::new(8)),
    };
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/version", get(version))
        .route("/v1/processing-states", get(processing_states))
        .route("/v1/memory/proposals", post(create_proposal))
        .route(
            "/v1/memory/proposals/{proposal_id}/review",
            post(review_proposal),
        )
        .route("/v1/maps/drafts", post(propose_map_draft))
        .route(
            "/v1/maps/drafts/{map_version_id}/review",
            post(review_map_draft),
        )
        .route("/v1/entities", post(propose_entity))
        .route("/v1/entities/{entity_id}/review", post(review_entity))
        .route("/v1/entities/identity-groups", get(identity_groups))
        .route("/v1/relationships", post(propose_relationship))
        .route(
            "/v1/relationships/{relationship_id}/review",
            post(review_relationship),
        )
        .route("/v1/graph/query", post(graph_query))
        .route("/v1/cross-map/mappings", post(propose_cross_map_mapping))
        .route(
            "/v1/cross-map/mappings/{mapping_id}/review",
            post(review_cross_map_mapping),
        )
        .route("/v1/search", post(search))
        .route(
            "/openapi.json",
            get(|| async { ApiDoc::openapi().to_json().unwrap() }),
        )
        .with_state(state)
}

#[tokio::main]
async fn main() {
    if std::env::args().any(|arg| arg == "--openapi") {
        println!(
            "{}",
            ApiDoc::openapi()
                .to_pretty_json()
                .expect("OpenAPI serialization cannot fail")
        );
        return;
    }

    let repository = match std::env::var("ENGRAVE_DATABASE_URL") {
        Ok(database_url) => Some(Arc::new(
            PgRepository::connect(&database_url)
                .await
                .expect("connect PostgreSQL"),
        )),
        Err(_) => None,
    };
    let lance = match std::env::var("ENGRAVE_LANCEDB_PATH") {
        Ok(path) => Some(Arc::new(
            LanceProjectionAdapter::connect(&path, "memory_projection")
                .await
                .expect("connect LanceDB"),
        )),
        Err(_) => None,
    };
    let bind_address = format!(
        "127.0.0.1:{}",
        std::env::var("ENGRAVE_API_PORT").unwrap_or_else(|_| "3000".into())
    );
    let listener = tokio::net::TcpListener::bind(&bind_address)
        .await
        .expect("bind API listener");
    axum::serve(listener, app_with_retrieval(repository, lance))
        .await
        .expect("API server failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_is_read_only_and_versioned() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/v1/processing-states")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/v1/version")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }

    #[test]
    fn openapi_contains_phase_a_routes() {
        let json = ApiDoc::openapi().to_json().unwrap();
        assert!(json.contains("/v1/health"));
        assert!(json.contains("/v1/version"));
        assert!(json.contains("/v1/processing-states"));
    }

    #[tokio::test]
    async fn proposal_route_requires_actor_and_never_activates_on_capture() {
        let response = app().oneshot(Request::builder().method("POST").uri("/v1/memory/proposals").header("content-type", "application/json").body(Body::from(serde_json::json!({"proposer":"ignored","claim":"Use UTC","reason":"explicit","scope":"personal","applies_when":"always","evidence":["chunk:1"],"policy":"personal_area"}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), 401);
        let response = app().oneshot(Request::builder().method("POST").uri("/v1/memory/proposals").header("x-actor-id", "actor-1").header("content-type", "application/json").body(Body::from(serde_json::json!({"proposer":"ignored","claim":"Use UTC","reason":"explicit","scope":"personal","applies_when":"always","evidence":["chunk:1"],"policy":"personal_area"}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(String::from_utf8(body.to_vec())
            .unwrap()
            .contains("pending"));
    }

    fn json_request(
        method: &str,
        uri: &str,
        actor: Option<&str>,
        body: serde_json::Value,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(actor) = actor {
            builder = builder.header("x-actor-id", actor);
        }
        builder.body(Body::from(body.to_string())).unwrap()
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn map_draft_requires_actor_and_publication_is_immutable() {
        let app = app();
        let unauthorized = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/maps/drafts",
                None,
                serde_json::json!({
                    "tenant_id": uuid::Uuid::new_v4(),
                    "area_id": uuid::Uuid::new_v4(),
                    "reason": "initial sales map",
                    "definition": {"kinds": [{"key":"account","label":"Account"}], "relations": []},
                    "policy": "personal_area",
                    "predecessor": null,
                }),
            ))
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), 401);

        let propose = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/maps/drafts",
                Some("agent"),
                serde_json::json!({
                    "tenant_id": uuid::Uuid::new_v4(),
                    "area_id": uuid::Uuid::new_v4(),
                    "reason": "initial sales map",
                    "definition": {"kinds": [{"key":"account","label":"Account"}], "relations": []},
                    "policy": "personal_area",
                    "predecessor": null,
                }),
            ))
            .await
            .unwrap();
        assert_eq!(propose.status(), 200);
        let map_version_id = body_json(propose).await["map_version_id"]
            .as_str()
            .unwrap()
            .to_string();

        let publish = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/maps/drafts/{map_version_id}/review"),
                Some("agent"),
                serde_json::json!({"expected_version": 1, "idempotency_key": "publish", "action": "approve"}),
            ))
            .await
            .unwrap();
        assert_eq!(publish.status(), 200);
        assert_eq!(body_json(publish).await["published"], true);

        // Immutability: editing after publication must be rejected, not
        // silently accepted.
        let edit_after_publish = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/maps/drafts/{map_version_id}/review"),
                Some("agent"),
                serde_json::json!({
                    "expected_version": 2,
                    "idempotency_key": "edit-after-publish",
                    "action": {"edit": {"kinds": [{"key":"account","label":"Account"}], "relations": []}},
                }),
            ))
            .await
            .unwrap();
        assert_eq!(edit_after_publish.status(), 422);
    }

    #[tokio::test]
    async fn map_review_rejects_stale_version_with_conflict() {
        let app = app();
        let propose = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/maps/drafts",
                Some("agent"),
                serde_json::json!({
                    "tenant_id": uuid::Uuid::new_v4(),
                    "area_id": uuid::Uuid::new_v4(),
                    "reason": "initial sales map",
                    "definition": {"kinds": [{"key":"account","label":"Account"}], "relations": []},
                    "policy": "personal_area",
                    "predecessor": null,
                }),
            ))
            .await
            .unwrap();
        let map_version_id = body_json(propose).await["map_version_id"]
            .as_str()
            .unwrap()
            .to_string();

        let stale = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/maps/drafts/{map_version_id}/review"),
                Some("agent"),
                serde_json::json!({"expected_version": 99, "idempotency_key": "publish", "action": "approve"}),
            ))
            .await
            .unwrap();
        assert_eq!(stale.status(), 409);
    }

    #[tokio::test]
    async fn entity_and_relationship_kind_validation_is_enforced() {
        let app = app();
        let map_version_id = uuid::Uuid::new_v4();
        let area_id = uuid::Uuid::new_v4();
        let tenant_id = uuid::Uuid::new_v4();

        let unknown_kind = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/entities",
                Some("agent"),
                serde_json::json!({
                    "tenant_id": tenant_id, "area_id": area_id, "map_version_id": map_version_id,
                    "proposer": "ignored", "reason": "capture", "kind": "widget",
                    "descriptor": {}, "origin": "observed", "policy": "personal_area",
                    "governing_kinds": ["account", "person"],
                }),
            ))
            .await
            .unwrap();
        assert_eq!(unknown_kind.status(), 422);

        let unknown_relation = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/relationships",
                Some("agent"),
                serde_json::json!({
                    "tenant_id": tenant_id, "area_id": area_id, "map_version_id": map_version_id,
                    "proposer": "ignored", "reason": "capture",
                    "source_entity_id": uuid::Uuid::new_v4(), "target_entity_id": uuid::Uuid::new_v4(),
                    "relation_kind": "owns", "origin": "observed", "policy": "personal_area",
                    "governing_relations": [],
                }),
            ))
            .await
            .unwrap();
        assert_eq!(unknown_relation.status(), 422);
    }

    #[tokio::test]
    async fn cross_map_requires_independent_reviewer_and_blocked_is_terminal() {
        let app = app();
        let propose = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/cross-map/mappings",
                Some("agent"),
                serde_json::json!({
                    "tenant_id": uuid::Uuid::new_v4(),
                    "source_area_id": uuid::Uuid::new_v4(),
                    "target_area_id": uuid::Uuid::new_v4(),
                    "source_map_version_id": uuid::Uuid::new_v4(),
                    "target_map_version_id": uuid::Uuid::new_v4(),
                    "relation": "related_to",
                    "rationale": "shared account concept",
                    "proposer": "ignored",
                }),
            ))
            .await
            .unwrap();
        assert_eq!(propose.status(), 200);
        let mapping_id = body_json(propose).await["cross_map_mapping_id"]
            .as_str()
            .unwrap()
            .to_string();

        let self_approve = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/cross-map/mappings/{mapping_id}/review"),
                Some("agent"),
                serde_json::json!({"expected_version": 1, "idempotency_key": "approve", "action": {"approve": {"expires_at": null}}}),
            ))
            .await
            .unwrap();
        assert_eq!(self_approve.status(), 403);

        let approve = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/cross-map/mappings/{mapping_id}/review"),
                Some("reviewer"),
                serde_json::json!({"expected_version": 1, "idempotency_key": "approve", "action": {"approve": {"expires_at": null}}}),
            ))
            .await
            .unwrap();
        assert_eq!(approve.status(), 200);

        let block = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/cross-map/mappings/{mapping_id}/review"),
                Some("reviewer"),
                serde_json::json!({"expected_version": 2, "idempotency_key": "block", "action": {"block": {"reason": "no longer trusted"}}}),
            ))
            .await
            .unwrap();
        assert_eq!(block.status(), 200);

        // Blocked is permanently terminal: nothing can move it anywhere else.
        let approve_after_block = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/cross-map/mappings/{mapping_id}/review"),
                Some("reviewer"),
                serde_json::json!({"expected_version": 3, "idempotency_key": "re-approve", "action": {"approve": {"expires_at": null}}}),
            ))
            .await
            .unwrap();
        assert_eq!(approve_after_block.status(), 422);
    }

    #[tokio::test]
    async fn graph_query_is_bounded_area_scoped_and_never_leaks_across_areas() {
        let app = app();
        let area_a = uuid::Uuid::new_v4();
        let area_b = uuid::Uuid::new_v4();
        let map_version_id = uuid::Uuid::new_v4();
        let tenant_id = uuid::Uuid::new_v4();

        async fn propose_and_approve_entity(
            app: &Router,
            tenant_id: uuid::Uuid,
            area_id: uuid::Uuid,
            map_version_id: uuid::Uuid,
            kind: &str,
            idempotency_key: &str,
        ) -> String {
            let propose = app
                .clone()
                .oneshot(json_request(
                    "POST",
                    "/v1/entities",
                    Some("agent"),
                    serde_json::json!({
                        "tenant_id": tenant_id, "area_id": area_id, "map_version_id": map_version_id,
                        "proposer": "ignored", "reason": "capture", "kind": kind,
                        "descriptor": {}, "origin": "observed", "policy": "personal_area",
                        "governing_kinds": ["account", "person"],
                    }),
                ))
                .await
                .unwrap();
            assert_eq!(propose.status(), 200);
            let entity_id = body_json(propose).await["entity_id"]
                .as_str()
                .unwrap()
                .to_string();
            let approve = app
                .clone()
                .oneshot(json_request(
                    "POST",
                    &format!("/v1/entities/{entity_id}/review"),
                    Some("agent"),
                    serde_json::json!({"expected_version": 1, "idempotency_key": idempotency_key, "action": "approve"}),
                ))
                .await
                .unwrap();
            assert_eq!(approve.status(), 200);
            entity_id
        }

        let account =
            propose_and_approve_entity(&app, tenant_id, area_a, map_version_id, "account", "a1")
                .await;
        let person =
            propose_and_approve_entity(&app, tenant_id, area_a, map_version_id, "person", "a2")
                .await;
        let foreign =
            propose_and_approve_entity(&app, tenant_id, area_b, map_version_id, "account", "b1")
                .await;

        let relationship_propose = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/relationships",
                Some("agent"),
                serde_json::json!({
                    "tenant_id": tenant_id, "area_id": area_a, "map_version_id": map_version_id,
                    "proposer": "ignored", "reason": "capture",
                    "source_entity_id": person, "target_entity_id": account,
                    "relation_kind": "owns", "origin": "observed", "policy": "personal_area",
                    "governing_relations": [{"key": "owns", "label": "Owns", "source_kind": "person", "target_kind": "account"}],
                }),
            ))
            .await
            .unwrap();
        assert_eq!(relationship_propose.status(), 200);
        let relationship_id = body_json(relationship_propose).await["relationship_id"]
            .as_str()
            .unwrap()
            .to_string();
        let relationship_approve = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/relationships/{relationship_id}/review"),
                Some("agent"),
                serde_json::json!({"expected_version": 1, "idempotency_key": "r1", "action": "approve"}),
            ))
            .await
            .unwrap();
        assert_eq!(relationship_approve.status(), 200);

        // Traversal: starting from the person in Area A reaches the account.
        let traverse = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/graph/query",
                None,
                serde_json::json!({"area_id": area_a, "start": [person]}),
            ))
            .await
            .unwrap();
        assert_eq!(traverse.status(), 200);
        let packet = body_json(traverse).await;
        let node_ids: Vec<String> = packet["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["entity_id"].as_str().unwrap().to_string())
            .collect();
        assert!(node_ids.contains(&person));
        assert!(node_ids.contains(&account));
        assert!(!node_ids.contains(&foreign));

        // Leakage: querying Area A with a start id that only exists in Area
        // B must return nothing, even though the id is real and approved —
        // Area boundaries are never crossed implicitly.
        let leakage_attempt = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/graph/query",
                None,
                serde_json::json!({"area_id": area_a, "start": [foreign]}),
            ))
            .await
            .unwrap();
        assert_eq!(leakage_attempt.status(), 200);
        let leaked_packet = body_json(leakage_attempt).await;
        assert!(leaked_packet["nodes"].as_array().unwrap().is_empty());

        // Bounded: a tiny node budget truncates rather than returning everything.
        let bounded = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/graph/query",
                None,
                serde_json::json!({"area_id": area_a, "start": [person], "max_nodes": 1}),
            ))
            .await
            .unwrap();
        let bounded_packet = body_json(bounded).await;
        assert_eq!(bounded_packet["nodes"].as_array().unwrap().len(), 1);
        assert_eq!(bounded_packet["truncated"], true);
    }
}
