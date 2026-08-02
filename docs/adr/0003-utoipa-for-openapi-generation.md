# ADR-0003: Utoipa for OpenAPI generation

## Status

Accepted

## Context

`13-api-principles.md` requires that the OpenAPI description be "a tested
artifact" — CI fails when the committed description and the code disagree —
and names **Utoipa** or **Aide** as candidates. `06-service-architecture.md`
additionally requires that `smash-contracts` be "the source for generated
schemas": every surface (API, Next.js client, MCP tool descriptions, future
SDKs) must agree on one origin for wire types, not five independently
hand-maintained copies.

## Decision

Use **Utoipa** for OpenAPI generation, with `smash-contracts` types as the
single origin of the published description.

Contract types in `smash-contracts` derive `utoipa::ToSchema`; request/response
DTOs and path operations in `smash-api` are annotated with Utoipa's
`#[utoipa::path(...)]` macro and collected into a single `utoipa::OpenApi`
document assembled in the `api` crate. That document — not a hand-written
YAML file — is the published contract; a CI step (`cargo run --bin
generate-openapi` or equivalent, wired once real endpoints exist) regenerates
it and diffs against the committed copy, failing the build on drift (the
"OpenAPI drift check" placeholder in the Phase A CI pipeline; see
`.github/workflows/rust-ci.yml`).

Utoipa was chosen over Aide primarily because its schema derivation attaches
directly to the same struct that Serde and Garde already annotate
(`#[derive(Serialize, Deserialize, Validate, ToSchema)]` on one type in
`smash-contracts`), keeping "one struct, one source of truth" intact across
serialization, validation, and documentation. Aide's `OperationIo`/schema
generation is comparable in capability but is more idiomatically paired with
`axum::extract` types built inline in handler modules, which pulls schema
definition toward `smash-api` and away from `smash-contracts` — working
against the "contracts is the single origin" requirement above.

## Consequences

- `smash-contracts` depends on `utoipa` (no `axum` feature — schema
  derivation does not require the web framework). This keeps schema
  definition in the framework-free layer, consistent with ADR-0004.
- `smash-api` assembles the `OpenApi` document from `smash-contracts` types
  plus its own path annotations; it owns the "publish this as `/openapi.json`
  and diff it in CI" concern.
- MCP tool schemas (`smash-mcp`) and any future generated SDKs read from the
  same `smash-contracts` types rather than re-deriving their own shape,
  avoiding drift between the HTTP contract and the MCP tool contract.
- Phase A ships no real endpoints, so the OpenAPI drift check in CI is
  currently a documented no-op placeholder (see the CI workflow) — it
  becomes a real gate once `smash-api` has its first route in a later
  phase.

## Alternatives rejected and why

- **Aide** — a capable, actively maintained alternative built specifically
  around `axum::extract`, with less boilerplate for handlers that build
  their schema inline. Rejected because it pulls schema ownership toward
  handler code in `smash-api` rather than toward `smash-contracts`, which
  conflicts with the "contracts crate types are the single origin of the
  published description" requirement this ADR exists to satisfy.
- **Hand-maintained OpenAPI YAML/JSON**, generated from nothing — rejected
  outright: `13-api-principles.md` explicitly requires the description be a
  tested, code-derived artifact, not a document that can silently drift from
  the implementation.
