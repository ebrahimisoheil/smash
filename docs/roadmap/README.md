# ENGRAVE V2 Roadmap

> **Status:** implementation source of truth for the V2 workspace
> **Audience:** founder, contributors, design partners, and future maintainers
> **Scope:** Community Edition first; managed scale, SSO, and enterprise operations second
> **Excludes:** delivery dates, sprint estimates, and file-by-file coding instructions

This roadmap defines what ENGRAVE V2 is, why it should exist, which parts of the current ENGRAVE implementation should survive, which architectural boundaries should change, and how the new system should be built in capability phases.

It is the implementation source of truth for this repository, decomposed into
working documents. Historical source material is maintained outside the
repository boundary.

## Reading order

### Why

| Doc | Contents |
|---|---|
| [00 — Purpose](00-purpose.md) | What this roadmap is, V1/V2 relationship, V1 classification rule |
| [01 — Product thesis](01-product-thesis.md) | Agent memory control plane, category positioning |
| [02 — Philosophy](02-philosophy.md) | Storage is not memory; humans define meaning; the reason is part of the record; forgetting is a feature; one contract; bounded context |
| [03 — Product language](03-product-language.md) | Internal concept → product language mapping |

### What

| Doc | Contents |
|---|---|
| [04 — Core domain model](04-domain-model.md) | Tenant, roles, Area, Map, Cross-Map, Source, chunk, entity, Memory, Proposal, Rule, Event, AI run |
| [05 — Canonical storage responsibilities](05-storage-responsibilities.md) | PostgreSQL, MinIO, LanceDB, tenant provisioning and placement |
| [06 — Service architecture](06-service-architecture.md) | Rust/Axum backend, crate selection, worker, Next.js, Docker Compose |
| [07 — Source ingestion](07-source-ingestion.md) | Source classes, state machine, safety boundary, proposal generation |
| [08 — Memory write and upsert](08-memory-write-upsert.md) | Deterministic write pipeline, duplicates, contradictions, supersession |
| [09 — Retrieval architecture](09-retrieval-architecture.md) | Light search, Aggressive search, Cross-Map retrieval, ranking evaluation |
| [10 — Rules and harness enforcement](10-rules-harness.md) | Evaluation points, effects, precedence, rule test harness |
| [11 — Agent session contract](11-agent-session-contract.md) | The portable agent loop |
| [12 — MCP, skills, prompts, connectors](12-mcp-skills-prompts-connectors.md) | MCP server and consumer, Registry, skills, prompts, native connectors |
| [13 — API principles](13-api-principles.md) | Versioning, commands vs queries, uploads, OpenAPI |
| [14 — Web application requirements](14-web-application.md) | Home, Areas, Library, Review, saved record, search, mobile |
| [15 — Authentication, authorization, security](15-auth-security.md) | Authorization model, threat list, release security gates |
| [16 — Observability and operations](16-observability-operations.md) | Operational telemetry vs decision ledger, decision envelope, AI Tracer, replay, health, backups |
| [17 — Testing and evaluation](17-testing-evaluation.md) | Contract, storage, retrieval, security, end-to-end, UI tests |
| [18 — V1 capabilities to preserve or reuse](18-v1-capabilities.md) | Preserve / reuse / do-not-copy lists |

### How

| Doc | Contents |
|---|---|
| [19 — Implementation phases](19-phases.md) | Phase gates A–J overview |
| [`phases/`](phases/README.md) | One document per phase with scope and acceptance criteria |
| [20 — Post-Community managed service](20-managed-service.md) | Multi-tenancy, SSO, governance, connectors, billing, decision intelligence |
| [21 — Definition of product success](21-product-success.md) | North star, activation, measures, vanity metrics to avoid |
| [22 — Architectural decision summary](22-decision-summary.md) | The 20 normative V2 decisions |
| [23 — Architecture diagrams](23-diagrams.md) | 16 contract diagrams |
| [24 — Primary technical references](24-references.md) | Upstream docs and V1 behavioral reference |

## The one-sentence positioning

> Notion stores what a team writes. Jira tracks what a team does. ENGRAVE governs what its agents remember.

## The backend stack in one line

> **Axum + Tokio + Tower + SQLx + Serde + Utoipa**

Full crate selection: [00 — Purpose](00-purpose.md#backend-crate-selection) and [06 — Service architecture §6.1](06-service-architecture.md#61-rust-and-axum-are-the-product-backend).

## The phase rule

Each phase is a capability gate. Do not begin a later phase merely because work has started; begin it when the previous phase's invariants and acceptance criteria are met. Parallel experimentation is allowed, but the main branch preserves a coherent product.
