use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use engrave_contracts::{ProposalId, SourceState};
use engrave_core::{MemoryStore, ProposalInput, ReviewAction};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
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
    paths(health, version, processing_states),
    components(schemas(HealthResponse, VersionResponse, ProcessingState, SourceState)),
    info(title = "ENGRAVE V2 API", version = env!("CARGO_PKG_VERSION"))
)]
struct ApiDoc;

fn app() -> Router {
    let state = AppState {
        memories: Arc::new(Mutex::new(MemoryStore::default())),
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

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("bind API listener");
    axum::serve(listener, app())
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
