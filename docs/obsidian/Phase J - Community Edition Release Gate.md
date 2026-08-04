# Next-agent prompt — Phase J Community Edition release gate

Copy the prompt below into the next agent task.

```text
You are the next ENGRAVE V2 implementation agent. Phase I bounded aggressive
search is implemented locally, but its deferred evidence is carried into this
phase. Your mission is to work on Phase J: the Community Edition release gate.

Read these before editing:

1. docs/roadmap/19-phases.md
2. docs/roadmap/phases/phase-j-community-edition-release-gate.md
3. docs/roadmap/phases/phase-i-aggressive-search.md
4. docs/phase-i-ledger.md
5. docs/roadmap/09-retrieval-architecture.md
6. docs/roadmap/15-auth-security.md
7. docs/roadmap/16-observability-operations.md
8. docs/roadmap/17-testing-evaluation.md
9. docs/obsidian/MCP Registry.md
10. crates/core, crates/api, crates/mcp, crates/storage, crates/worker,
    migrations, compose.yaml, plugin, and existing release scripts

Current truth:

- Phase H MCP stdio, Rules, Areas, PostgreSQL persistence, and the connector
  boundary are implemented and verified locally.
- Phase I aggressive search executes bounded decomposition, retrieval,
  reranking, Cross-Map/graph traversal, Source inspection, optional explicit
  connector inspection, contradiction reporting, and Rule-gated disclosure.
- Phase I evidence is recorded in docs/phase-i-ledger.md.
- This is not yet a production-readiness or public-release claim.

Guided workspace and ontology interview:

- Add a single broad `workspace_setup` capability rather than many narrowly
  named ontology tools. It must support `begin`, `draft`, `confirm`, `submit`,
  inspect, and cancel actions through one stable schema.
- The agent may interview a user about their work, stakeholders, domains,
  terminology, and relationships, but narrative understanding is not an
  authorization mechanism and must not create or activate anything silently.
- `begin` returns the user’s authorized Area choices from PostgreSQL plus an
  explicit `request_new_area` option. Users may request an Area; they cannot
  grant themselves access. Area administrators or the governed review path
  decide access.
- `draft` returns a structured ontology preview containing Areas, kinds,
  relationships, assumptions, unresolved questions, and exact source/context
  references. The preview must be understandable as chat text and available
  as structured JSON/UI metadata for clients that render forms or cards.
- `confirm` must be explicit and attributable to the current actor, agent,
  host, session, tenant, and purpose. `submit` creates only a Map/Area
  Proposal or draft; publication and access grants remain separate reviewed
  operations.
- Clients without rich UI support must receive deterministic Markdown/JSON
  fallback. Do not require XML, magic tags, or model-produced markup.
- Interview state, confirmations, proposal links, and cancellations must be
  durable, replay-safe, tenant-scoped, and auditable.

Phase I gaps that must remain visible and be closed or explicitly accepted:

- Run a corpus-level Light-vs-Aggressive evaluation; the current fixture only
  proves one deterministic contradiction-exposure delta.
- Add a live worker test that changes active authorization between stages and
  proves later stages observe the narrowed Rule state.
- Add an approved Cross-Map and graph fixture to the live worker pipeline test
  and assert a durable Traverse step with bounded results.
- Treat external connector calls, OAuth, remote MCP, Registry publication,
  artifact verification, and security review as release-gate work, not as
  implied by local tests.

Additional missing product flows that must be designed explicitly:

- invitations with expiring, single-use, tenant-bound tokens and acceptance;
- Area access requests with reviewer decisions, grants, denial, expiry, and
  audit history;
- admin membership/Area-grant management;
- a pre-persistence source policy that can reject forbidden uploads, plus a
  post-extraction quarantine/redaction boundary;
- a source decision flow where the user chooses Source-only, create a Memory
  Proposal, submit for review, or approve immediately only when authorized.

Existing `memberships`, `area_grants`, Source quarantine, Memory Proposals, and
Rule gates are primitives, not proof that these product workflows exist.

Phase J objective:

Make the Community Edition installable, documented, backed up, upgradeable,
and honest about its deployment boundaries. A non-maintainer must be able to
install it, add Sources, review Memory, connect two agents, retrieve through
Light and Aggressive modes, inspect provenance, enforce a Rule, back up data,
and upgrade through migrations without repository knowledge.

Required release work:

- produce versioned containers and prebuilt MCP binaries for supported
  platforms;
- provide reproducible Compose installation and configuration documentation;
- verify SQLx migration, backup, restore, and upgrade paths;
- document Light and Aggressive behavior, budgets, provenance, cancellation,
  partial results, proposals, and known limits;
- close or explicitly disposition every Phase I carried gap above;
- validate connector credential isolation, revocation, timeout, retry, and
  quarantine behavior;
- generate and validate Registry metadata only from a tagged release;
- verify artifact checksums, package contents, environment variables, and
  absence of secrets;
- complete the security, rollback, backup/restore, and release review.

Hard boundaries:

- Do not weaken the engrave-core Rule evaluator or shared gateway.
- Do not move authorization into prompts, skills, release scripts, or UI.
- Do not activate Memory from aggressive search or release automation.
- Do not publish remote MCP or Registry metadata until OAuth, audience,
  tenant isolation, audit, rate limits, revocation, package verification, and
  owner approval are proven.
- Do not call local green tests production readiness.

Tests are part of the implementation:

- disposable clean-machine Compose install and upgrade tests;
- backup/restore and migration rollback tests;
- Light/Aggressive benchmark and contradiction-quality evaluation;
- live stale-authorization and live worker Traverse tests;
- connector leakage, prompt injection, credential, revocation, and timeout
  adversarial tests;
- MCP stdio startup/shutdown and artifact checksum tests;
- Registry schema and package metadata verification tests.

Before declaring Phase J complete:

1. inspect the current worktree and update the plan;
2. run cargo fmt --all;
3. run cargo test --workspace;
4. run cargo clippy --workspace --all-targets -- -D warnings;
5. run every relevant ignored PostgreSQL and release/integration suite against
   disposable infrastructure;
6. verify a clean-machine install, backup/restore, and migration upgrade;
7. record exact commands, artifacts, checksums, and evidence in a Phase J
   ledger;
8. inspect git diff --check and list any remaining release blockers.

Do not publish, claim production readiness, or mark Phase J complete while any
release blocker is merely assumed or indirectly inferred.
```
