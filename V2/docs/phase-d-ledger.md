# Phase D Ledger — Memory Lifecycle and Review

This is the durable Phase D session ledger. The checkout did not contain the
Obsidian `ENGRAVE V2/Plans/Phase D Progress.md` or `ENGRAVE V2/Phases/Phase D.md`
notes, so this repository ledger is the source of truth until those notes are
restored.

## Session 1 — proposal-first lifecycle implementation

**Status:** implementation complete; verification passed. Commit reference is
recorded below.

**Decisions**

- Every capture creates a `pending` Proposal; capture never creates an active
  Memory, including Personal-Area policy.
- Personal-Area proposals require confirmation. Shared and Cross-Map proposals
  require an independent reviewer; the proposer cannot approve their own shared
  proposal.
- Exact normalized duplicate candidates and deterministic explicit negation
  contradiction candidates are review metadata, never silent merges.
- Review and lifecycle mutations require the reviewed version and an
  idempotency key. Replays return the original result; stale versions fail with
  a conflict.
- PostgreSQL activation is guarded by `app.memory_admission = 'approved'` and
  the storage adapter sets that flag only after the proposal CAS succeeds.
- Activity includes action, actor, reason, evidence, and version transition;
  supersession preserves predecessor lineage and marks the old Memory
  `superseded`.
- Retrieval, LanceDB, Maps, Rules execution, MCP, billing, and hosted
  deployment remain out of scope.

**Open questions / owners**

- Restore the missing Obsidian Phase D ledger/work-plan/phase-note exports.
  Owner: repository maintainer.
- Replace the local-dev `X-Actor-Id` bridge with the shared Tower
  authentication/authorization layer before any network exposure. Owner:
  security/API follow-up.
- Add live PostgreSQL migration and transaction tests when the bundled Rust
  toolchain/Compose services are available. Owner: Phase D verification.

**Exact verification and evidence**

- `git branch --show-current` → `v2-publish-d`.
- `git diff --check` → pass.
- Core contract tests cover no activation on proposal, Personal vs Shared
  admission, duplicate/contradiction candidates, stale review conflict,
  idempotent approval, archive/restore/expire, and Activity evidence.
- API test covers unauthenticated proposal rejection and pending-only capture.
- `npm run build` in `V2/apps/web` is the UI verification command.
- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo fmt --all -- --check` →
  pass.
- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo test --workspace` → pass:
  API 3, contracts 2, core 9, Sales fixture 1, storage 3, worker 1, doc tests
  0 failures.
- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo clippy --workspace
  --all-targets -- -D warnings` → pass.
- `npm ci --ignore-scripts && npm run build` in `V2/apps/web` → pass; `/` and
  `/review` statically generated. `npm audit` reports 3 high-severity package
  findings; no dependency upgrade was included in Phase D.

**Commit reference:** `e440806a` (`Implement Phase D governed Memory review lifecycle`).
