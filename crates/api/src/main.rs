use axum::{routing::get, Json, Router};
use serde::Serialize;
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
    paths(health, version),
    components(schemas(HealthResponse, VersionResponse)),
    info(title = "SMASH V2 API", version = env!("CARGO_PKG_VERSION"))
)]
struct ApiDoc;

fn app() -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/version", get(version))
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
    }
}
