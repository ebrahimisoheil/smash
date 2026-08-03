# ADR-0002: Garde for edge validation

## Status

Accepted

## Context

`13-api-principles.md` requires input validation "at the edge, before
application services" and lists **Garde** or **Validator** as the two
candidates the roadmap already narrowed to. Both are derive-macro-based
validation crates for Rust structs and are broadly comparable on the basics
(string length, numeric ranges, required fields, email/URL formats).

ENGRAVE's edge validation is not purely structural, though. Several of the
validation rules the roadmap anticipates are **context-aware**: a Rule body's
validity depends on which tenant it belongs to, and a Memory or Map mutation's
validity depends on the Map's current version (optimistic concurrency,
`13-api-principles.md`). A validation rule that only sees the struct being
validated — with no way to reach into request-scoped context like tenant ID
or an expected version — cannot express these checks at the edge; they would
have to be pushed down into the service layer, which is exactly the layering
the roadmap wants to avoid ("validation... before application services").

## Decision

Use **Garde** for edge validation across the workspace.

Garde's validation context (`#[garde(context(...))]` / the `Validate` trait's
associated `Context` type) lets a validator function receive an arbitrary
context value passed in by the caller at validation time — e.g. the resolved
tenant ID from a Tower auth layer, or the Map's expected version extracted
from an `If-Match`-style header — and use it inside a custom or built-in
validation rule. This is the deciding factor: it allows genuinely
context-aware edge checks ("this Rule reference is valid only within this
tenant's Rule set") without smuggling application-service logic into the
handler layer, and without validating twice (once shallow at the edge, once
"for real" in the service).

## Consequences

- Contract types in `engrave-contracts` derive `garde::Validate` where request
  bodies need validation; Axum handlers in `engrave-api` call `.validate(&ctx)`
  with a context built from Tower-layer-resolved request state (tenant,
  expected Map version, etc.) before invoking application services.
- `engrave-contracts` depends on `garde` with the `derive` feature. This is a
  small, framework-independent dependency — it does not violate `core`'s
  framework-free constraint (ADR-0004) since context-aware validation is
  itself edge/contract concern, not domain logic.
- The team commits to Garde's context-passing idiom as the standard shape for
  any validator that needs more than the struct's own fields; ad-hoc
  hand-rolled validation functions bypassing Garde are a review flag.

## Alternatives rejected and why

- **Validator** — the more established crate at the time of writing, with a
  larger set of built-in validators out of the box. Rejected because its
  context story is weaker: `validator`'s custom-validation functions receive
  only the field value being validated (or, with more recent versions, a
  fixed struct-level context type declared once per struct), which does not
  cleanly support the "same struct, different valid context depending on
  which tenant/Map-version is in scope" shape ENGRAVE needs without resorting
  to constructing a different struct per context or manual second-pass
  validation in the service layer — reintroducing the layering problem this
  ADR exists to avoid.
- **Hand-rolled validation functions** (no crate) — rejected as the default
  because it forfeits derive-macro ergonomics for the common cases (length,
  range, required) while not meaningfully simplifying the context-aware
  cases Garde already handles.
