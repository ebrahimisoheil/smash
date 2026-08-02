use axum::{routing::get, Json, Router};
use serde::Serialize;
use smash_contracts::SourceState;
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
        service: "smash-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(OpenApi)]
#[openapi(
    paths(health, version, processing_states),
    components(schemas(HealthResponse, VersionResponse, ProcessingState, SourceState)),
    info(title = "SMASH V2 API", version = env!("CARGO_PKG_VERSION"))
)]
struct ApiDoc;

fn app() -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/version", get(version))
        .route("/v1/processing-states", get(processing_states))
        .route(
            "/openapi.json",
            get(|| async { ApiDoc::openapi().to_json().unwrap() }),
        )
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
}
