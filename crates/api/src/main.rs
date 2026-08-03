use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use engrave_contracts::{ProposalId, SourceState};
use engrave_core::{
    light_search, ApplicationError, CircuitBreaker, DegradedMode, DenseHit,
    DeterministicEmbeddingProvider, EmbeddingConfiguration, EmbeddingProvider, EmbeddingVector,
    MemoryStore, ProjectionIdentity, ProposalInput, QueryEmbeddingCache, ReviewAction,
    SearchProfile, SearchRequest as CoreSearchRequest,
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
}
