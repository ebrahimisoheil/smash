# ENGRAVE V2 Workspace

This repository is the Engrave V2 workspace. Legacy V1 sources are not part of
this repository; historical behavior is preserved only where explicitly
documented in the V2 contracts and classification notes.

## What V2 is

V2 is a deliberate transition from a local file-oriented memory tool into an open-source, service-oriented **agent memory control plane** that can later become a managed multi-user product without changing its conceptual contract.

The initial V2 implementation is:

- **Rust / Axum** — product backend, authorization boundary, orchestration
- **Tokio** — async runtime for API, worker, and connectors
- **Next.js** — human web application
- **PostgreSQL** — canonical database, accessed through SQLx
- **MinIO** — S3-compatible object storage
- **LanceDB** — rebuildable vector and multimodal retrieval sidecar (native Rust)
- **Docker Compose** — Community Edition deployment unit

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

Skills, prompts, MCP, connectors, and agent-host integrations sit on top of this foundation.

## Where the plan lives

All planning, philosophy, architecture, and phase documentation lives in [`docs/roadmap/`](docs/roadmap/README.md).

Start with [`docs/roadmap/README.md`](docs/roadmap/README.md).

The roadmap in `docs/roadmap/` is the implementation source of truth for this
repository. Historical source material is intentionally maintained outside the
repository boundary.

The Engrave rename is a clean local cutover. It uses fresh `engrave_*` named
volumes; existing `smash_*` volumes are left untouched and are not adopted
automatically. Export/import is required if old local data must be carried
forward.

## Classification rule for anything brought forward from V1

Every feature brought forward from V1 must be classified as one of:

1. **Contract to preserve** — user-visible or agent-visible behavior that is already correct and valuable.
2. **Reference implementation** — useful code or tests that may be adapted, but whose storage or runtime assumptions must not constrain V2.
3. **Historical experiment** — evidence about what worked or failed, without requiring the original implementation to survive.
4. **Legacy surface** — a feature that stays in V1 and is not rebuilt until V2 proves a current user need.

See [`docs/roadmap/18-v1-capabilities.md`](docs/roadmap/18-v1-capabilities.md).
