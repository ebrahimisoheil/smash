# ADR-0004: Crate names and dependency direction

## Status

Accepted

## Context

`06-service-architecture.md` §6.1 fixes the property that matters most for
the Rust workspace's long-term health: "`core` is framework-free and every
surface reuses it." It sketches a suggested crate shape (`core`, `storage`,
`api`, `worker`, `mcp`, `contracts`) but defers the exact names and
dependency directions to Phase A. This ADR settles both, as the concrete
workspace scaffolded in session A1.

Naming matters because these crate names appear in `Cargo.lock`, in binary
names shipped in the Docker image, and in every future import statement —
renaming later is a real cost. Dependency direction matters because it is
the only thing standing between "core is framework-free" as a stated
intention and "core is framework-free" as a fact a new contributor can rely
on without re-auditing the whole tree.

## Decision

The workspace at `V2/` has six crates plus two non-Cargo units:

| Path | Crate name | Depends on | Must NOT depend on |
|---|---|---|---|
| `V2/crates/contracts` | `engrave-contracts` | serde, uuid, time, garde, utoipa | axum, sqlx, tokio |
| `V2/crates/core` | `engrave-core` | `engrave-contracts`, thiserror, async-trait | axum, tower, sqlx, reqwest, lancedb |
| `V2/crates/storage` | `engrave-storage` | `engrave-core`, sqlx, aws-sdk-s3, lancedb | axum |
| `V2/crates/api` | `engrave-api` | `engrave-core`, `engrave-storage`, axum, tower, utoipa | — |
| `V2/crates/worker` | `engrave-worker` | `engrave-core`, `engrave-storage`, tokio | axum |
| `V2/crates/mcp` | `engrave-mcp` | `engrave-core`, `engrave-storage` | axum |
| `V2/apps/web` | (Next.js, not a Cargo crate) | — | direct DB access |
| `V2/eval` | fixtures/benchmark dir, not a Cargo crate | — | — |

The dependency direction is a strict DAG:
`engrave-contracts` ← `engrave-core` ← `engrave-storage` ← {`engrave-api`,
`engrave-worker`, `engrave-mcp`}. Nothing depends "sideways" or "up" — `api`,
`worker`, and `mcp` never depend on each other, and nothing below `storage`
ever depends on anything above it.

**`core` is framework-free, enforced by `cargo deny`, not convention.**
`V2/deny.toml` bans `axum` and `tower-http` from the resolved dependency
graph everywhere except as a direct dependency of `engrave-api` (verified
empirically: today those two crates have exactly one dependent each in the
whole graph — `engrave-api` — so the ban has zero false positives). This is a
CI failure, not a review habit: session A1's acceptance check is planting an
`axum` import in `engrave-core`, confirming `cargo deny check` fails, then
removing it.

The rest of the "must not depend on" column (sqlx/tokio/reqwest/lancedb
staying out of `contracts`/`core`) is enforced structurally by the DAG shape
above plus a dedicated CI step that walks `cargo tree -p engrave-core` and
`cargo tree -p engrave-contracts`, asserting none of those crate names appear
in either subtree — see the "check-core-boundary" job in
`.github/workflows/rust-ci.yml` and the long comment in `V2/deny.toml`
explaining why a blanket `cargo-deny` ban on those names is not the right
tool (they are legitimately several hops deep in `engrave-storage`'s own
dependency tree, e.g. `lancedb` vendors `reqwest` via
`lance-namespace-reqwest-client`).

`engrave-api` and `engrave-worker` produce the `api` and `worker` binaries
respectively, built from one dependency-cached multi-stage `Dockerfile`
(`V2/Dockerfile`). `engrave-mcp` produces an `mcp` binary but is not yet wired
into the Dockerfile — it ships once Phase-later work defines its deployment
shape.

## Consequences

- A contributor can answer "can I add X here?" by reading one table instead
  of re-deriving intent from the roadmap prose each time.
- Adding a legitimate new dependency to `engrave-api` (e.g. a new Tower layer)
  never requires touching `deny.toml`. Adding `axum` to any crate other than
  `engrave-api` breaks CI immediately, by design.
- `engrave-storage` is the only crate allowed to know about Postgres, S3, or
  LanceDB concretely; `engrave-core` defines the ports (traits) that
  `engrave-storage` implements, once Phase B introduces real domain services.
  Phase A ships no ports yet — that is deliberately out of scope here.
- `V2/apps/web` and `V2/eval` are placeholders: the former is a
  build-verifying Next.js skeleton with no real UI, the latter is a
  near-empty fixtures directory. Neither has Cargo Rust code, so neither is
  a workspace member and neither is bound by the dependency-direction table.
- Renaming any crate after this ADR is a new ADR, not a quiet refactor,
  because the names are load-bearing in `Cargo.lock`, Docker binary names,
  and CI job names.

## Alternatives rejected and why

- **Single monolithic crate** (`engrave`) with modules instead of crates —
  rejected because Rust's dependency graph is enforced at the crate
  boundary, not the module boundary; a `mod api { use axum; }` inside one
  crate cannot be mechanically prevented from being visible to `mod core`
  the way `cargo deny` can prevent `engrave-core`'s `Cargo.toml` from ever
  compiling with `axum` in it. The framework-free invariant needs a crate
  boundary to be enforceable at all.
- **`domain`/`app` split within `core`** (separating pure domain types from
  application services into two crates) — the roadmap's suggested shape
  keeps them together as one `core` crate; splitting further is deferred
  until Phase B's real domain services reveal whether the split earns its
  keep, rather than guessing now with no code to observe.
- **`worker` and `mcp` sharing one crate** (since both depend on exactly
  `engrave-core` + `engrave-storage` today) — rejected because they are
  separate deployables with separate lifecycles (the worker is a long-running
  job-loop process; MCP is a stdio/HTTP adapter process), and the roadmap
  treats them as distinct services in `06-service-architecture.md` §6.2 and
  the MCP integration doc (`12-mcp-skills-prompts-connectors.md`).
- **Enforcing the full "must not depend on" table via `cargo-deny` bans
  alone** — tried first, rejected after empirical testing (`cargo tree -i
  reqwest` showed `reqwest` reachable from `engrave-storage` through four
  layers of `lance-*` crates with no single stable "wrapper" boundary,
  meaning a global ban would need to enumerate every intermediate vendor
  crate and would break on the next `lancedb` version bump). The DAG shape
  plus a targeted `cargo tree -p engrave-core` CI check achieves the same
  guarantee without that fragility.
