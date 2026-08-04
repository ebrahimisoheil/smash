# Phase H evidence ledger

Status: implemented locally; not a production-readiness claim.

## Delivered

- `crates/mcp` is a transport-neutral JSON-RPC MCP service with a stdio binary
  and a PostgreSQL-backed adapter selected by `ENGRAVE_DATABASE_URL`.
- `initialize`, `tools/list`, `tools/call`, `resources/list`,
  `resources/read`, `prompts/list`, and `prompts/get` are deterministic.
- The six-tool surface is `status`, `recall`, `propose_memory`,
  `ingest_source`, `review`, and `rules`.
- Calls require tenant, actor, agent, session, Area, purpose, and role context.
- Calls pass the Phase G `PreToolGateway`; block and approval are returned as
  structured errors outside model behavior.
- The backend port includes durable decision recording with policy envelope,
  outcome, actor/agent/session identity, and credential-free argument hash
  fields. Rule versions are loaded per call.
- `engrave-core::connector` plus `engrave-storage::NotionConnector` provide a
  read-heavy connector contract and an
  idempotent Source-version ledger with tenant isolation, permission narrowing,
  cursor propagation, and deletion tombstones.
- `plugin/` contains separate host packaging, six thin versioned skills, five
  commands, and stdio MCP configuration.

## Verification evidence

```text
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p engrave-core -p engrave-mcp
DATABASE_URL=postgres://... cargo test -p engrave-storage --tests -- --ignored
DATABASE_URL=postgres://... cargo test -p engrave-mcp --test live_phase_h -- --ignored
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize"}' | cargo run -q -p engrave-mcp
python3 scripts/generate-phase-h-metadata.py 0.1.0
```

The workspace suite passed. The local disposable PostgreSQL container also
passed the ignored storage and MCP live suites, including the two-agent,
reviewed-Memory, proposal-only, durable-decision, and connector-version proofs.
The metadata generator intentionally emits no package entry until the current
official Registry schema and the exact release artifact package type are
confirmed.

## Deferred by design

- Streamable HTTP, OAuth, protected-resource metadata, scopes, audience binding,
  revocation, and HTTP `401`/`403` behavior.
- OAuth authorization flow and encrypted credential-store implementation for
  the first connector require the managed secret store; the connector client
  accepts only a tenant-bound opaque credential instance.
- Two real vendor hosts, release artifact checksums, Registry schema
  validation, and publication from a tagged release.

These are explicit Phase H follow-ups or release-gate work. Phase I aggressive
search was not started.
