# 22 — Architectural Decision Summary

> Source: the historical roadmap source §23

The following decisions are **normative for V2 unless replaced by an explicit architecture decision record**.

| # | Decision | Detail |
|---|---|---|
| 1 | PostgreSQL is canonical for structured, transactional, and lifecycle state. | [05 §5.1](05-storage-responsibilities.md#51-postgresql-is-canonical) |
| 2 | MinIO stores original Source bytes and large artifacts through S3-compatible semantics. | [05 §5.2](05-storage-responsibilities.md#52-minio-owns-binary-objects) |
| 3 | LanceDB is a rebuildable vector and multimodal retrieval sidecar. | [05 §5.3](05-storage-responsibilities.md#53-lancedb-is-a-rebuildable-retrieval-sidecar) |
| 4 | **Rust with Axum** owns the product API and application orchestration: **Axum + Tokio + Tower + SQLx + Serde + Utoipa**. | [06 §6.1](06-service-architecture.md#61-rust-and-axum-are-the-product-backend) |
| 5 | A separate worker executes long-running jobs from the same Rust workspace and the same framework-free core crate. | [06 §6.2](06-service-architecture.md#62-worker-uses-the-backends-core-crate) |
| 6 | Next.js is the human application and does not duplicate backend business rules. | [06 §6.3](06-service-architecture.md#63-nextjs-is-the-human-application) |
| 7 | Docker Compose is the Community Edition deployment unit. | [06 §6.4](06-service-architecture.md#64-docker-compose-is-the-community-edition-product-unit) |
| 8 | Memory is a governed claim, distinct from Sources, chunks, entities, and vectors. | [02 §2.1](02-philosophy.md#21-storage-is-not-memory) |
| 9 | Durable writes are review-gated unless an explicit admission policy permits them. | [08](08-memory-write-upsert.md) |
| 10 | Maps are versioned Area contracts; Cross-Map mappings connect them conservatively. | [04 §4.4–4.5](04-domain-model.md#44-map) |
| 11 | Light search is the default; Aggressive search is deliberate, budgeted, and traced. | [09](09-retrieval-architecture.md) |
| 12 | Rules are enforced mechanically outside the model. | [10](10-rules-harness.md) |
| 13 | MCP is an adapter, connector, and distribution layer — **not the product moat**. | [12](12-mcp-skills-prompts-connectors.md) |
| 14 | Indexes and projections are disposable; canonical records and evidence are portable. | [05 §5.3](05-storage-responsibilities.md#53-lancedb-is-a-rebuildable-retrieval-sidecar) |
| 15 | Community Edition and managed ENGRAVE share the same domain contract. | [02 §2.5](02-philosophy.md#25-local-ownership-and-managed-convenience-share-one-contract) |
| 16 | Managed PostgreSQL comes up once with a shared schema and mandatory tenant isolation; enterprises are provisioned as **data and policy**, not hand-built databases. | [05 §5.4](05-storage-responsibilities.md#54-tenant-provisioning-and-placement) |
| 17 | LanceDB is tenant-scoped through a trusted namespace and placement record; users receive roles inside the tenant rather than personal vector stores. | [05 §5.3](05-storage-responsibilities.md#53-lancedb-is-a-rebuildable-retrieval-sidecar) |
| 18 | Enterprise Admin and AI Governance Admin are **customer** roles with tenant-wide oversight according to policy; ENGRAVE platform operators have no default customer-content access. | [15](15-auth-security.md) |
| 19 | AI runs, decision envelopes, Rule evaluations, tool calls, and outcomes are canonical tenant product records, separate from sampled operational telemetry. | [16](16-observability-operations.md) |
| 20 | Future AI Tracer, replay, and decision analytics must be possible from V2 records **without requiring raw cross-tenant surveillance**. | [16 §16.3–16.4](16-observability-operations.md#163-ai-tracer-and-replay) |
| 21 | The domain and application core is a **framework-free Rust crate**. Axum, the worker, and the MCP adapter are surfaces over it; none of them owns Memory logic or authorization. | [06 §6.1](06-service-architecture.md#61-rust-and-axum-are-the-product-backend) |

## Backend crate selection

Normative defaults for decision 4:

| ENGRAVE concern | Rust choice |
|---|---|
| API framework | Axum |
| Async runtime | Tokio |
| Middleware | Tower and tower-http |
| JSON serialization | Serde |
| Request validation | Garde or Validator |
| PostgreSQL | SQLx |
| Migrations | SQLx migrations |
| OpenAPI generation | Utoipa or Aide |
| Authentication / JWT | jsonwebtoken plus OIDC libraries |
| HTTP / connectors | Reqwest |

## Changing a decision

A decision on this list changes only through an explicit architecture decision record created before code depends on the change. See [Phase A](phases/phase-a-contract-and-workspace-foundation.md).
