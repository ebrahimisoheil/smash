# Contributing to ENGRAVE V2

This covers the Engrave V2 workspace at the repository root.

## Before you start

Read `docs/roadmap/06-service-architecture.md` and
`docs/roadmap/13-api-principles.md` first. They are the normative source
for the backend stack and API shape; this file covers process, not
architecture.

## Architecture Decision Records (ADRs)

Every decision that changes a normative choice — a framework, a crate
boundary, a persistence technology, an API shape commitment — gets an ADR
under `docs/adr/`, one file per decision, numbered sequentially
(`000N-short-title.md`).

Use this format:

```markdown
# ADR-000N: Title

## Status
(proposed | accepted | superseded by ADR-000M)

## Context
What forces are in tension. What roadmap section this touches.

## Decision
What we're doing, stated plainly.

## Consequences
What becomes easier, what becomes harder, what new constraints this creates.

## Alternatives rejected and why
Every real alternative considered, and the specific reason it lost — not
just "didn't fit," but the concrete property that made it lose.
```

An ADR is accepted once it is merged; there is no separate approval step
beyond normal code review. Superseding an ADR means writing a new one that
says so in its own Status line and updating the old ADR's Status to point at
it — never silently deleting or rewriting an accepted ADR.

When in doubt about whether something needs an ADR: if reverting the
decision later would require touching more than one crate, or would break a
guarantee another crate's tests rely on, write the ADR.

## The crate boundary rule

The workspace shape and dependency direction are fixed in
[ADR-0004](docs/adr/0004-crate-boundaries-and-dependency-direction.md).
The one rule every contributor must internalize:

> **`engrave-core` is framework-free. It compiles against `engrave-contracts`,
> `thiserror`, and `async-trait` — nothing else.**

Concretely, **never add any of the following to `crates/core/Cargo.toml`**:

- `axum` or `tower` / `tower-http` — HTTP framework and its middleware belong
  in `engrave-api` only.
- `sqlx` — a SQL driver is a storage-adapter concern (`engrave-storage`).
- `reqwest` — outbound HTTP is an adapter concern, not domain logic.
- `lancedb` — retrieval-sidecar access is a storage-adapter concern.

If your feature genuinely needs one of these from within domain logic, the
domain logic is in the wrong crate, or it needs a port (a trait defined in
`engrave-core`, implemented in `engrave-storage`) rather than a direct
dependency. This is not a style preference — `axum` and `tower-http` reaching
`engrave-core` is a hard CI failure via `cargo deny check` (`deny.toml`),
and `sqlx`/`tokio`/`reqwest`/`lancedb` reaching `engrave-core` or
`engrave-contracts` is caught by the `check-core-boundary` CI job that walks
`cargo tree -p engrave-core`. Don't try to route around either check; fix the
layering instead.

The same rule, one level up: `engrave-contracts` stays free of `axum`, `sqlx`,
and `tokio` too — it is the framework-free source of wire types shared by
every surface, including future non-Rust clients that generate from its
schemas.

## Fixture-change rules

Test fixtures for retrieval, rule evaluation, and eval benchmarks live under
`eval/`. That directory is a near-empty placeholder as of Phase A — no
canonical fixture exists yet.

Once the canonical **Sales fixture** (the reference tenant dataset used
across retrieval, rules, and benchmark evaluation — introduced in a later
phase, see `docs/roadmap/17-testing-evaluation.md`) exists:

- Changes to it are reviewed as carefully as schema migrations — every
  downstream benchmark and test that asserts against fixture content is a
  potential silent regression if the fixture shifts under it.
- A fixture change that alters expected retrieval or rule-evaluation output
  must update the corresponding golden/expected files in the same PR, with
  the diff called out explicitly in the PR description — never as an
  incidental side effect of an unrelated change.
- New fixture data should extend the existing Sales fixture rather than
  introducing a second parallel fixture set, unless the roadmap doc calls
  for genuinely different domain coverage (e.g. a second industry vertical).

This section will be expanded with concrete fixture-format documentation
once the Sales fixture lands — don't invent format details here ahead of
that.

## Local checks

Before pushing, run what CI runs:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

`cargo deny` and `cargo audit` require their local databases
(`cargo deny fetch`, or just run `cargo deny check` once online) — CI fetches
these fresh on every run; locally they cache under `~/.cargo`.

`SQLX_OFFLINE=true` is required for normal builds once SQLx query macros are
in use (Phase B+) — there is no live schema in Phase A, so this doesn't yet
apply to anything in the tree, but set it in your shell profile now so it's
already true when it starts mattering.
