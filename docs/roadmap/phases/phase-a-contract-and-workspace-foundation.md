# Phase A — Contract and Workspace Foundation

> Source: the historical roadmap source §20, Phase A

## Goal

Establish the meaning of the system before any code depends on it.

## Scope

Define:

- the V2 workspace and contribution rules;
- architecture decision records;
- terminology;
- domain schemas;
- API conventions;
- the event model;
- the error model;
- the configuration strategy.

### The canonical Sales fixture

Create **one** canonical Sales fixture containing:

- accounts;
- people;
- an opportunity;
- a call;
- a PDF;
- an approved decision;
- a contradiction;
- a superseding decision;
- an Area Rule;
- a Cross-Map proposal to Marketing.

**This fixture becomes the common language for tests, UI, and demos.**

### Crate boundaries

Establish the Cargo workspace and the boundaries between:

- domain / application core (**framework-free**);
- infrastructure adapters (SQLx, S3, LanceDB);
- Axum API surface;
- worker;
- MCP adapter;
- shared contracts;
- Next.js application;
- evaluation assets.

Suggested shape: [06 §6.1](../06-service-architecture.md#suggested-workspace-shape). Names are settled here; what is fixed is that **`core` does not depend on Axum** and every surface reuses it.

Decide which schemas are **generated** and which are **authored**, while keeping the API source of truth unambiguous. The Rust types in `contracts` and the Utoipa-generated OpenAPI description must not be able to disagree silently.

### Backend crate selection

Ratify the normative crate list — **Axum, Tokio, Tower/tower-http, Serde, Garde or Validator, SQLx, SQLx migrations, Utoipa or Aide, jsonwebtoken, Reqwest** — as architecture decision records, and resolve the two either/or choices (validation crate, OpenAPI crate) before code depends on them.

Set up from the start: Cargo workspace layout, committed `Cargo.lock`, SQLx offline query cache, `cargo clippy` with warnings denied, `cargo audit` and `cargo deny` in CI, and the multi-stage container build.

## Acceptance criteria

- [ ] Every core object has a documented lifecycle and ownership boundary.
- [ ] No unresolved ambiguity exists between Source, chunk, entity, Memory, Proposal, and Rule.
- [ ] Events and idempotency are part of every mutation contract.
- [ ] Tenant, enterprise role, agent identity, AI run, and decision-envelope identities are present in the contract.
- [ ] The fixture can express all critical lifecycle cases.
- [ ] V1 features are mapped to preserve / reuse / defer / retire.
- [ ] Architecture decisions are recorded **before** code depends on them.
- [ ] The Cargo workspace builds, and `core` compiles without any web-framework dependency.
- [ ] The backend crate selection is ratified, with the validation and OpenAPI choices resolved.

## References

- [04 — Core domain model](../04-domain-model.md)
- [03 — Product language](../03-product-language.md)
- [13 — API principles](../13-api-principles.md)
- [18 — V1 capabilities to preserve or reuse](../18-v1-capabilities.md)
- [22 — Architectural decision summary](../22-decision-summary.md)
