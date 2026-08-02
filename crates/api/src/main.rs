use axum::{http::StatusCode, routing::get, Json, Router};
use serde::Serialize;
use smash_storage::{wait_for_tcp_endpoint, PgRepository};
use utoipa::{OpenApi, ToSchema};

#[derive(Debug, Serialize, ToSchema)]
struct HealthResponse {
    status: &'static str,
    readiness: &'static str,
}

#[derive(Clone, Debug)]
struct RuntimeConfig {
    database_url: String,
    object_store_endpoint: String,
    object_store_bucket: String,
    port: u16,
}

impl RuntimeConfig {
    fn from_env() -> Result<Self, String> {
        let required = |name: &str| {
            std::env::var(name).map_err(|_| format!("missing required configuration: {name}"))
        };
        Ok(Self {
            database_url: required("SMASH_DATABASE_URL")?,
            object_store_endpoint: required("SMASH_MINIO_ENDPOINT")?,
            object_store_bucket: required("SMASH_MINIO_BUCKET")?,
            port: std::env::var("SMASH_API_PORT")
                .unwrap_or_else(|_| "3000".to_owned())
                .parse()
                .map_err(|_| "SMASH_API_PORT must be a valid port".to_owned())?,
        })
    }
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
        readiness: "ready",
    })
}

#[utoipa::path(get, path = "/v1/readiness", responses((status = 200, body = HealthResponse), (status = 503, body = HealthResponse)))]
async fn readiness() -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok",
            readiness: "ready",
        }),
    )
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
    paths(health, readiness, version),
    components(schemas(HealthResponse, VersionResponse)),
    info(title = "SMASH V2 API", version = env!("CARGO_PKG_VERSION"))
)]
struct ApiDoc;

fn app() -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/readiness", get(readiness))
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

    let config = RuntimeConfig::from_env().expect("invalid SMASH API configuration");
    if std::env::args().any(|arg| arg == "--migrate") {
        PgRepository::connect_and_migrate(&config.database_url)
            .await
            .expect("PostgreSQL is not ready or migrations failed");
        return;
    }
    let _repository = PgRepository::connect(&config.database_url)
        .await
        .expect("PostgreSQL is not ready");
    wait_for_tcp_endpoint(&config.object_store_endpoint, 20)
        .await
        .expect("MinIO is not ready");
    assert!(!config.object_store_bucket.is_empty());
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port))
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
        assert!(json.contains("/v1/readiness"));
    }
}
