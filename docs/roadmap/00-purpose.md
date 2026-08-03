# 00 — Purpose of This Roadmap

> Source: the historical roadmap source §1

This roadmap defines what ENGRAVE V2 is, why it should exist, which parts of the current ENGRAVE implementation should survive, which architectural boundaries should change, and how the new system should be built in capability phases.

It is intentionally more durable than a backlog. A backlog answers what the team will do next. This roadmap answers what the product means and what must remain true as its implementation changes.

## V1 and V2 coexist

The V2 workspace is created alongside the current workspace. V1 remains a working reference implementation and a source of proven functionality. V2 copies behavior deliberately, not architecture accidentally.

Every feature brought forward is classified as exactly one of:

| Classification | Meaning |
|---|---|
| **Contract to preserve** | User-visible or agent-visible behavior that is already correct and valuable. |
| **Reference implementation** | Useful code or tests that may be adapted, but whose storage or runtime assumptions must not constrain V2. |
| **Historical experiment** | Evidence about what worked or failed, without requiring the original implementation to survive. |
| **Legacy surface** | A feature that stays in V1 and is not rebuilt until V2 proves a current user need. |

The concrete lists are in [18 — V1 capabilities to preserve or reuse](18-v1-capabilities.md).

## V2 is not a cosmetic rewrite

V2 is a deliberate transition from a local file-oriented memory tool into an open-source, service-oriented **agent memory control plane** that can later become a managed multi-user product without changing its conceptual contract.

## Initial technology commitment

| Layer | Choice |
|---|---|
| Product backend | Rust / Axum |
| Async runtime | Tokio |
| Web application | Next.js |
| Canonical database | PostgreSQL |
| Object storage | MinIO (S3-compatible) |
| Vector / multimodal retrieval sidecar | LanceDB |
| Community Edition deployment unit | Docker Compose |

Skills, prompts, MCP, connectors, and agent-host integrations sit on top of this foundation.

### Backend crate selection

The backend stack in one line:

> **Axum + Tokio + Tower + SQLx + Serde + Utoipa**

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

These are the normative default choices. Each is replaceable only through an explicit architecture decision record — see [22 — Architectural decision summary](22-decision-summary.md).

LanceDB is itself written in Rust, so the retrieval sidecar is a native crate dependency rather than a foreign-language SDK. This does not change its status: it remains a **rebuildable projection**, never canonical.

## Scope boundaries of this document

**In scope:** product meaning, domain contracts, architectural boundaries, capability phases, acceptance criteria, normative decisions.

**Out of scope:** delivery dates, sprint estimates, file-by-file coding instructions.
