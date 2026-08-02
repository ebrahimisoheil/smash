# 24 — Primary Technical References

> Source: SMASH_V2.md §25

**The implementation verifies behavior against current upstream documentation rather than copying examples from secondary tutorials.**

## Upstream documentation

### Rust backend

| Topic | Reference |
|---|---|
| Axum | <https://docs.rs/axum/latest/axum/> |
| Tokio | <https://docs.rs/tokio/latest/tokio/> and <https://tokio.rs/tokio/tutorial> |
| Tower | <https://docs.rs/tower/latest/tower/> |
| tower-http | <https://docs.rs/tower-http/latest/tower_http/> |
| Serde | <https://serde.rs/> |
| Garde | <https://docs.rs/garde/latest/garde/> |
| Validator | <https://docs.rs/validator/latest/validator/> |
| SQLx | <https://docs.rs/sqlx/latest/sqlx/> |
| SQLx migrations and CLI | <https://docs.rs/sqlx/latest/sqlx/migrate/index.html> |
| Utoipa | <https://docs.rs/utoipa/latest/utoipa/> |
| Aide | <https://docs.rs/aide/latest/aide/> |
| jsonwebtoken | <https://docs.rs/jsonwebtoken/latest/jsonwebtoken/> |
| Reqwest | <https://docs.rs/reqwest/latest/reqwest/> |
| Cargo workspaces | <https://doc.rust-lang.org/cargo/reference/workspaces.html> |
| Rust container builds | <https://hub.docker.com/_/rust> |
| `cargo audit` / RustSec | <https://rustsec.org/> |
| `cargo deny` | <https://embarkstudios.github.io/cargo-deny/> |

### Platform and infrastructure

| Topic | Reference |
|---|---|
| Docker Compose dependency readiness | <https://docs.docker.com/compose/how-tos/startup-order/> |
| Next.js self-hosting | <https://nextjs.org/docs/app/guides/self-hosting> |
| PostgreSQL documentation | <https://www.postgresql.org/docs/> |
| PostgreSQL Row-Level Security | <https://www.postgresql.org/docs/17/ddl-rowsecurity.html> |
| PostgreSQL partitioning guidance | <https://www.postgresql.org/docs/current/ddl-partitioning.html> |
| MinIO container documentation | <https://min.io/docs/minio/container/index.html> |
| LanceDB tables and namespaces | <https://docs.lancedb.com/tables-and-namespaces> |
| LanceDB filtering | <https://docs.lancedb.com/search/filtering> |
| OpenTelemetry GenAI semantic conventions | <https://opentelemetry.io/docs/specs/semconv/registry/attributes/gen-ai/> |
| MCP Registry publishing | <https://modelcontextprotocol.io/registry/quickstart> |
| MCP authorization | <https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization> |

## V1 behavioral reference

The existing SMASH repository remains the behavioral reference for V1 contracts:

- [`README.md`](../../../README.md)
- [`ARCHITECTURE.md`](../../../ARCHITECTURE.md)
- [`benchmarks/RESULTS.md`](../../../benchmarks/RESULTS.md)
- [`docs/memory-contract.html`](../../../docs/memory-contract.html)
- [`docs/mcp.html`](../../../docs/mcp.html)
- [`docs/scale.html`](../../../docs/scale.html)
- the current tests under [`tests/`](../../../tests/)

## Source document

- [`SMASH_V2.md`](../../../SMASH_V2.md) — the original combined V2 document from which this roadmap is derived.
