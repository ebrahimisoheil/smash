# ADR-0001: Backend stack ratification

## Status

Accepted

## Context

ENGRAVE V2 replaces the V1 Python CLI/local server with a fresh Rust workspace
(`V2/`). Before any crate is scaffolded, the team needs one settled answer for
the core building blocks — HTTP framework, async runtime, middleware,
serialization, database driver/migrations, auth, and outbound HTTP — so every
crate is written against the same primitives from the first commit instead of
accreting inconsistent choices crate-by-crate.

The roadmap (`V2/docs/roadmap/06-service-architecture.md` §6.1 and
`V2/docs/roadmap/13-api-principles.md`) already names a normative crate
selection sourced from `SMASH_V2.md` §7. This ADR ratifies that selection as
the binding stack decision for the workspace scaffolded in session A1.

## Decision

The backend stack is:

| Concern | Choice |
|---|---|
| API framework | **Axum** |
| Async runtime | **Tokio** (one runtime shared by API, worker, and connector I/O) |
| Middleware | **Tower** and **tower-http** (auth, tracing, timeouts, concurrency limits, compression, request IDs as composable layers) |
| Serialization | **Serde** |
| PostgreSQL driver | **SQLx** |
| Migrations | **SQLx migrations** (forward-only from the first commit; no model auto-generation in production) |
| Authentication / JWT | **jsonwebtoken** plus OIDC libraries |
| Outbound HTTP | **Reqwest** (connectors, MCP servers, model providers) |

Edge validation (Garde) and OpenAPI generation (Utoipa) are large enough
decisions in their own right to warrant separate ADRs — see ADR-0002 and
ADR-0003.

These choices are normative for the whole workspace. Replacing any one of
them requires a new ADR, not an ad-hoc substitution in a single crate.

## Consequences

- Every crate that touches HTTP, async I/O, or the database uses the same
  primitives; there is one way to write a handler, one way to run a
  background task, one way to talk to Postgres.
- `engrave-core` stays framework-free by construction: none of Axum, Tower,
  SQLx, Reqwest, or LanceDB may appear in its dependency tree. This is
  enforced mechanically by `cargo deny` (`V2/deny.toml`) for Axum/tower-http
  and by the crate-graph shape itself (see ADR-0004) for the rest.
- SQLx's compile-time query checking requires either a live database or an
  offline query cache (`SQLX_OFFLINE=true` plus `.sqlx/`) to build in CI —
  addressed in ADR-0004's companion CI pipeline, and revisited properly once
  Phase B introduces the canonical schema.
- Adopting Tokio workspace-wide means every crate that spawns async work
  (api, worker, mcp) shares one runtime model; there is no risk of mixing
  async runtimes (e.g. async-std) in a single binary.

## Alternatives rejected and why

- **Actix-web** instead of Axum — more mature ecosystem at the time of
  writing, but its actor-based extractor model and separate runtime
  integration story are a worse fit for a framework-free core crate that
  must stay reusable from worker and MCP contexts without pulling in a web
  framework's actor system.
- **Diesel** instead of SQLx — Diesel's compile-time query builder is
  attractive, but SQLx's `query!` macro checks raw SQL directly against a
  live schema (or offline cache) without an ORM abstraction layer, which
  matches the roadmap's preference for explicit adapters over generated
  model code (`06-service-architecture.md`: "no model auto-generation in
  production").
- **async-graphql** / GraphQL surface instead of a REST-shaped Axum API —
  rejected because `13-api-principles.md` commits to a versioned,
  resource-oriented HTTP API with cursor pagination and idempotency keys;
  GraphQL's flexible query shape works against the "one canonical contract,
  tested as an artifact" principle.
- **Paseto** instead of JWT for tokens — JWT plus OIDC libraries were chosen
  for interoperability with managed SSO providers introduced later; Paseto
  has a smaller ecosystem of off-the-shelf OIDC integrations.
