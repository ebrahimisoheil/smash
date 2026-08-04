use engrave_mcp::{run_stdio, InMemoryBackend, McpService, PgMcpBackend};
use engrave_storage::PgRepository;
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Hosted OAuth/HTTP is deliberately not declared in Phase H. The same
    // service can receive another transport once its auth contract exists.
    if let Ok(database_url) = std::env::var("ENGRAVE_DATABASE_URL") {
        let repository = PgRepository::connect(&database_url)
            .await
            .map_err(|_| std::io::Error::other("unable to connect to PostgreSQL"))?;
        let service = McpService::new(
            Arc::new(PgMcpBackend {
                repository: Arc::new(repository),
            }),
            Vec::new(),
        )
        .map_err(std::io::Error::other)?;
        run_stdio(service).await
    } else {
        let service = McpService::new(Arc::new(InMemoryBackend::default()), Vec::new())
            .map_err(std::io::Error::other)?;
        run_stdio(service).await
    }
}
