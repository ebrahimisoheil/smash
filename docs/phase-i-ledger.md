# Phase I evidence ledger

Status: bounded aggressive-search implementation completed locally; this is not a production-readiness claim.

## Delivered

- `engrave-core::aggressive` requires explicit intent and carries tenant, actor,
  host, agent, session, Area, purpose, task, and query identity.
- Core hard budgets cover steps, elapsed time, tokens, candidates, and external
  calls. Step ordinals, authorized Areas, cancellation, timeout, partial,
  failure, contradiction, and uncertainty states are deterministic.
- Exact citation packets retain Memory or Source/Source-version/chunk,
  coordinate, and content-hash fields; source content is not treated as policy.
- Core contradiction detection is deterministic and citation-preserving. The
  evaluation fixture records Light exposure `0` and Aggressive exposure `1`
  for opposing renewal claims, retaining both Memory citations.
- PostgreSQL persists tenant-linked `search_traces` and `search_trace_steps`,
  with operation idempotency and RLS policies. Existing leased operations are
  reused for cancellation and worker checkpoints.
- API starts, inspects, and cancels bounded background investigations. MCP uses
  the existing `recall` tool with explicit `mode: aggressive` and commands for
  start/inspect/cancel; no new narrow tool family was added.
- API, MCP, and worker paths use the existing identity/Area boundary and core
  Rule evaluation. The worker re-evaluates before retrieval and persists a
  durable Rule decision, trace, citation set, and uncertainty when evidence is
  absent. Durable observations remain outside active Memory.
- Compose migration wiring includes `20260812100000_phase_i_aggressive_search.sql`.

## Verification evidence

```text
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
docker compose up -d postgres
docker compose exec -T postgres psql -U engrave_app -d engrave < migrations/20260812100000_phase_i_aggressive_search.sql
DATABASE_URL=postgres://engrave_app:engrave_local_only_change_me@localhost:5432/engrave cargo test -p engrave-storage --test live_phase_i -- --ignored
DATABASE_URL=postgres://engrave_app:engrave_local_only_change_me@localhost:5432/engrave cargo test -p engrave-storage --tests -- --ignored
DATABASE_URL=postgres://engrave_app:engrave_local_only_change_me@localhost:5432/engrave cargo test -p engrave-mcp --test live_phase_h -- --ignored
DATABASE_URL=postgres://engrave_app:engrave_local_only_change_me@localhost:5432/engrave cargo test -p engrave-worker -- --ignored --nocapture --test-threads=1
git diff --check
```

The workspace suite passed (including 83 core tests), clippy passed with
warnings denied, and `git diff --check` passed. The Phase I trace/graph proof,
the worker pipeline proof (including a local fake connector response and
prompt-like untrusted content), all ignored storage PostgreSQL suites, and the
Phase H MCP live proof passed against the disposable local PostgreSQL 16
container. The deterministic Light-vs-Aggressive contradiction fixture also
passed.

## Not yet proven / deferred release work

- The worker now executes bounded decomposition, repeated lexical retrieval,
  task-term-sensitive deterministic reranking, approved Cross-Map target
  narrowing, core bounded graph traversal, exact Source/chunk inspection,
  contradiction checks, untrusted-source warnings, and a separately
  Rule-gated disclosure step. Legacy aggressive operation payloads are routed
  into this same pipeline rather than the former lexical-only path.
  Direct external connector calls are not automatically issued by an
  investigation; connector-backed Sources are inspected through their
  canonical, already-synced Source-version/chunk lineage. Explicit connector
  calls are checked against the remaining time, step, and external-call
  budgets before I/O and are wrapped in the remaining time budget.
- A four-case credential-free Light-vs-Aggressive contradiction corpus now
  proves the contract-level quality delta; representative production-corpus
  quality remains outside local evidence.
- The live worker pipeline now activates a blocking Rule after the first
  retrieval decision and asserts that a later retrieval stage observes it.
- The live worker pipeline now seeds an approved Cross-Map mapping and graph
  entities and asserts a durable bounded `Traverse` step. Core and storage
  graph tests continue to cover the underlying traversal contract.
- Direct external connector calls are intentionally not started by an
  investigation unless a future connector-backed operation supplies an
  explicit connector request and credential reference; no connector call is
  made by the current canonical Source/chunk inspection path.
- External connector/OAuth hardening, remote MCP/public network exposure,
  Phase J release work, artifact verification, security review, production
  readiness, and Registry publication are not claimed.
