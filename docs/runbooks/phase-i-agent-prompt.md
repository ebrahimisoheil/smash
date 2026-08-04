# Next-agent prompt — Phase I aggressive search

Copy the prompt below into the next agent task.

```text
You are the next ENGRAVE V2 implementation agent. Phase H is complete locally;
your mission is to implement Phase I: bounded aggressive search.

Read these before editing:

1. docs/roadmap/19-phases.md
2. docs/roadmap/phases/phase-i-aggressive-search.md
3. docs/roadmap/09-retrieval-architecture.md
4. docs/roadmap/11-agent-session-contract.md
5. docs/roadmap/17-testing-evaluation.md
6. docs/roadmap/15-auth-security.md
7. docs/adr/0031-phase-g-rule-contract.md
8. docs/adr/0032-phase-g-mechanical-enforcement.md
9. docs/phase-h-ledger.md
10. crates/core, crates/api, crates/mcp, crates/storage, and crates/worker

Current truth:

- Phase H MCP stdio and PostgreSQL adapter are implemented.
- Every MCP tool loads active Rules per call, passes PreToolGateway, and
  applies a post-retrieval disclosure gate.
- The live proof covers two agent identities/hosts retrieving the same reviewed
  Memory, proposal-only session-end capture, durable decisions, and a
  versioned Notion-style connector Source sync.
- Phase H verification is recorded in docs/phase-h-ledger.md.
- Do not undo or duplicate the MCP/Rule/Area boundary.

Phase I objective:

Implement an explicit aggressive-search mode for multi-step investigation,
verification, and high-impact decisions. It must remain distinct from light
recall and must be bounded, attributable, reproducible, and safe.

Required properties:

- explicit user/agent intent to enter aggressive mode;
- a durable search trace linked to tenant, actor, host, agent, session, Area,
  purpose, and task;
- hard budgets for steps, time, tokens, candidates, and external calls;
- authorization and Rule evaluation before every retrieval, traversal,
  connector call, and disclosure;
- deterministic citation/provenance packets retaining exact Memory, Source,
  Source-version, and chunk references;
- contradiction and uncertainty reporting rather than silent synthesis;
- cancellation, timeout, partial-result, and failure states persisted durably;
- no silent Memory activation: durable observations remain Proposals;
- prompt injection and malicious Source content treated as untrusted data;
- no broad new family of narrowly named MCP tools.

Design constraints:

- Keep policy in engrave-core and the shared gateway. Prompts, skills, and
  search traces are not authorization mechanisms.
- Reuse the existing light-search, graph, Cross-Map, connector, queue, and
  decision-envelope contracts where appropriate.
- Background work belongs in the worker; interactive MCP calls should start or
  inspect bounded operations, not run unbounded investigations inline.
- Do not start Phase J, publish a remote MCP endpoint, or claim production
  readiness.

Tests are part of the implementation, not a follow-up. Add deterministic core
contract tests for budgets, step ordering, citations, contradictions,
authorization narrowing, cancellation, timeout, replay/idempotency, and
partial failure. Add thin MCP/API/worker translation tests and live PostgreSQL
tests against a disposable migrated database. Include adversarial tests for
Rule bypass, Area leakage, Cross-Map misuse, connector leakage, prompt
injection, budget exhaustion, stale authorization, and replay.

Before declaring completion:

1. update the plan and inspect the current worktree;
2. run cargo fmt --all;
3. run cargo test --workspace;
4. run cargo clippy --workspace --all-targets -- -D warnings;
5. run the relevant ignored PostgreSQL suites against a disposable database;
6. record exact commands and evidence in a Phase I ledger;
7. inspect git diff --check and explicitly list deferred release work.

Do not report success from unit tests alone. If a requirement is not proven,
keep working or state the exact blocker.
```
