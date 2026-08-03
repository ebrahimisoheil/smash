# 06 — Service Architecture

> Source: the historical roadmap source §7

## 6.1 Rust and Axum are the product backend

The backend is a Rust workspace. **Axum** exposes the canonical HTTP API, authentication boundary, authorization checks, orchestration, review workflows, retrieval router, rule evaluation, MCP integration endpoints, connector management, and health surfaces.

### Crate selection

> **Axum + Tokio + Tower + SQLx + Serde + Utoipa**

| ENGRAVE concern | Rust choice | Notes |
|---|---|---|
| API framework | **Axum** | Routing, extractors, typed handlers; the canonical HTTP surface |
| Async runtime | **Tokio** | One runtime shared by API, worker, and connector I/O |
| Middleware | **Tower** and **tower-http** | Auth, tracing, timeouts, concurrency limits, compression, request IDs as composable layers |
| JSON serialization | **Serde** | Request/response models and validated JSONB payloads |
| Request validation | **Garde** or **Validator** | Deterministic input checks at the edge, before application services |
| PostgreSQL | **SQLx** | Compile-time-checked queries against the canonical store |
| Migrations | **SQLx migrations** | Forward migrations from the first commit; no model auto-generation in production |
| OpenAPI generation | **Utoipa** or **Aide** | The published, tested API contract artifact |
| Authentication / JWT | **jsonwebtoken** plus OIDC libraries | Token verification, audience binding, managed SSO later |
| HTTP / connectors | **Reqwest** | Outbound calls to connectors, MCP servers, and model providers |

Choices on this list are normative. Replacing one requires an explicit architecture decision record.

### Properties the framework choice must not change

**Statelessness.** The API service is stateless except for short-lived in-process caches that are safe to discard. It does not own durable files on its container filesystem. It interacts with PostgreSQL, MinIO, and LanceDB through explicit adapters.

**Layering.** Business logic lives in a **framework-independent core crate** so worker tasks, MCP tools, and tests reuse the same contracts. The core crate must not depend on Axum. Axum handlers translate HTTP into application use cases and back; they do not contain authorization or Memory rules.

**Cross-cutting concerns are Tower layers, not handler code.** Authentication, tenant resolution, request IDs, idempotency-key capture, tracing spans, timeouts, and concurrency limits are middleware. A handler that re-implements one of these is a defect.

**No heavy work in request handlers.** Extraction, OCR, transcription, embedding, bulk migration, and aggressive-search workflows do not run in ordinary request handlers. API requests create Operations or Jobs and return stable identifiers. The worker executes long-running work. Lightweight validation and database mutations may complete inside the request.

**Never block the runtime.** CPU-bound work that must run in-process goes through a blocking-task pool rather than occupying an async executor thread. Long or unbounded work belongs in the worker regardless.

**Versioned from the beginning.** The Next.js application, MCP server, CLI, skills, and external clients use the same contracts. Internal endpoints may exist but must not become an undocumented second product API.

### Suggested workspace shape

| Crate | Responsibility |
|---|---|
| `core` | Domain and application services: Memory lifecycle, Rules, retrieval router, authorization decisions. No web framework, no SQL driver in its public API. |
| `storage` | SQLx, MinIO (S3), and LanceDB adapters implementing core's ports. |
| `api` | Axum router, Tower layers, Serde models, Utoipa annotations, error mapping. |
| `worker` | Tokio job loop over the same `core` and `storage` crates. |
| `mcp` | MCP adapter over `core`, sharing the API's authorization path. |
| `contracts` | Shared types, IDs, event and error models; the source for generated schemas. |

Exact crate names and boundaries are settled in [Phase A](phases/phase-a-contract-and-workspace-foundation.md). What is fixed here is that **`core` is framework-free and every surface reuses it.**

## 6.2 Worker uses the backend's core crate

The worker runs as a separate Docker Compose service, built from the **same versioned Rust workspace** — either the same image with a different entrypoint or a sibling binary from the same build. It polls or receives jobs, **claims them atomically**, updates progress, renews leases for long tasks, and records structured failures.

Worker responsibilities:

- Source extraction and normalization;
- OCR, transcript, and visual-description orchestration;
- stable chunk generation;
- embedding and LanceDB projection;
- entity and relationship proposal generation;
- Memory and Map proposal generation;
- connector synchronization;
- aggressive search substeps that exceed request budgets;
- exports, imports, backups, and reconciliation;
- retention and cleanup tasks.

**Job handlers must be idempotent.** A retry either recognizes completed output or replaces a derived version cleanly.

**The worker must not create active Memory merely because extraction succeeded.**

**The worker owns LanceDB indexing writes.** API processes query the index; only the worker mutates a tenant table. See [05 §5.3](05-storage-responsibilities.md#53-lancedb-is-a-rebuildable-retrieval-sidecar).

Extraction work that shells out to non-Rust tooling (OCR engines, transcription, office-format converters) runs as a supervised subprocess or a dedicated sidecar with explicit timeouts and resource limits — never as an unbounded in-process call. Its output is a derived artifact like any other, with processor name, version, and configuration fingerprint recorded.

## 6.3 Next.js is the human application

Next.js provides the user-facing interface. It is a **client of the Axum API contract**, not a second business-logic backend. Server Components may fetch data for initial rendering, but authorization decisions and mutations remain in the Rust backend.

Initial navigation stays small:

- Home
- Areas
- Library
- Review
- Rules
- settings and connectors in secondary navigation

The UI optimizes for inspecting Memory, understanding reasons, reviewing proposals, adding Sources, searching, and observing agent activity. It does **not** begin as a generic graph editor or database administration tool.

The Next.js container is self-hostable. In deployments exposed beyond localhost, place a reverse proxy or managed ingress in front of Next.js and the Axum API. Community Edition Compose may provide direct development ports but must document that TLS and public exposure require additional configuration.

Detail: [14 — Web application requirements](14-web-application.md).

## 6.4 Docker Compose is the Community Edition product unit

Default stack:

| Service | Role |
|---|---|
| `web` | Next.js application |
| `api` | Axum service (Rust) |
| `worker` | Background processing from the same Rust workspace |
| `postgres` | Canonical database |
| `minio` | S3-compatible object storage |
| migration/init | One-shot service running SQLx migrations, or an explicit migration command |
| reverse proxy | Optional profile for public self-hosting |
| model / connector profiles | Optional, only when needed |

**LanceDB** is an embedded **Rust crate dependency**, used by the API for queries and the worker for indexing, with a dedicated persistent volume or compatible object path. Because it is a native library rather than a network service, the architecture must define **writer ownership and file access explicitly**: the worker writes, the API reads.

### Rust build and image discipline

- Build `api` and `worker` from **one Cargo workspace** so they cannot drift apart in domain logic.
- Use a multi-stage Docker build with dependency-layer caching; ship a slim runtime image containing the compiled binaries, not the toolchain.
- Compile release binaries for the release artifact. Debug builds are not shipped.
- `Cargo.lock` is committed and is part of reproducibility, alongside pinned image digests.
- SQLx compile-time query checking requires either a reachable database or a committed offline query cache at build time. **Commit the offline cache** so CI and contributor builds do not depend on a live database.

### Startup discipline

- Compose health checks test **readiness**, not merely process existence.
- API and worker startup depend on healthy PostgreSQL and MinIO.
- Database migrations complete successfully before normal services accept work.
- Startup is **idempotent**: restarting the stack does not recreate buckets, duplicate data, or rerun destructive initialization.

### Reproducibility

- Pin container image versions or digests.
- Provide environment examples without real secrets.
- Named volumes are the default; bind mounts are an explicit development choice.
- **Backup and restore documentation is part of Community Edition completeness**, not an enterprise-only feature.
