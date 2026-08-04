# ENGRAVE V2 — Phase H Handoff

## Mission

Implement Phase H: MCP, skills, prompts, and one connector.

Phase G is the security boundary. Phase H must expose that boundary to agent
hosts without moving authorization into prompts, skills, or model behavior.

Do not start Phase I. Do not claim production readiness.

## Architecture decision

Keep two deliverables separate:

```text
mcp/
  Standalone MCP adapter. stdio first; Streamable HTTP later.

plugin/
  Host packaging: manifests, skills, commands, and MCP configuration.
```

The MCP server owns MCP tools, resources, prompts, schemas, transport, and
structured errors. `engrave-core` owns Rules, Areas, policy envelopes,
authorization, and the `PreToolGateway`. The plugin only teaches the host when
and how to use MCP capabilities.

Never duplicate policy logic in the plugin or skill text.

## Read first

1. `docs/roadmap/19-phases.md`
2. `docs/roadmap/phases/phase-h-mcp-skills-prompts-connector.md`
3. `docs/roadmap/12-mcp-skills-prompts-connectors.md`
4. `docs/roadmap/11-agent-session-contract.md`
5. `docs/roadmap/10-rules-harness.md`
6. `docs/roadmap/15-auth-security.md`
7. `docs/roadmap/17-testing-evaluation.md`
8. `docs/adr/0031-phase-g-rule-contract.md`
9. `docs/adr/0032-phase-g-mechanical-enforcement.md`
10. `docs/adr/0033-phase-g-fixture-and-rule-harness.md`
11. `docs/obsidian/MCP Registry.md`
12. Current `crates/core`, `crates/api`, `crates/mcp`, `crates/storage`, worker,
    migrations, fixtures, and benchmark code.

## MCP surface

Keep the model-facing surface small and stable:

| Capability | Purpose | Boundary |
|---|---|---|
| `status` | Session, tenant, Area, Rule, and connector health | Read-only |
| `recall` | Governed retrieval | Before retrieval + post-retrieval disclosure |
| `propose_memory` | Create a Memory Proposal only | Before proposal/write |
| `ingest_source` | Create or queue Source ingestion | Source and connector Rules |
| `review` | Review an eligible Proposal or governed item | Approval and role checks |
| `rules` | Inspect effective Rule metadata and envelope | Never expose secrets or hidden content |
| `resources/read` | Return governed context/evidence | Envelope and field filtering |

Every tool call must:

1. Resolve tenant, actor, agent identity, session, Area, purpose, role, and
   authorization context.
2. Resolve the current active Rule versions from PostgreSQL.
3. Evaluate the shared `RuleEvaluator`.
4. Invoke `PreToolGateway`.
5. Require explicit approval for `require_approval`.
6. Reject `block` outside the model.
7. Execute only after the gate passes.
8. Quarantine/redact tool results before returning them.
9. Record Rule ID/version, rationale, purpose, actor, outcome, argument hash,
   server identity, and approval metadata.

The model must never be the enforcement mechanism.

## Transport plan

### Local first

Implement stdio first for desktop agents and local CI. Do not add OAuth to
stdio. Credentials must come from the local environment or configured secret
store and must never be emitted in logs or MCP responses.

### Later hosted transport

Add Streamable HTTP behind a transport-neutral MCP service. Do not rewrite the
tool implementation when adding HTTP. HTTP MCP must use bearer-token
validation, protected-resource metadata, audience-bound tokens, scopes,
revocation, and `401`/`403` behavior required by the MCP authorization model.

## Plugin structure

```text
plugin/
├── .codex-plugin/plugin.json
├── .claude-plugin/plugin.json       optional
├── .cursor-plugin/plugin.json       optional
├── .mcp.json
├── skills/
│   ├── engrave-search/SKILL.md
│   ├── engrave-memory/SKILL.md
│   ├── engrave-rules/SKILL.md
│   ├── engrave-review/SKILL.md
│   ├── engrave-sources/SKILL.md
│   └── engrave-connectors/SKILL.md
├── commands/                        optional
└── tests/
    ├── unit/
    └── integration/
```

Skills should be thin, versioned, and explicit about mutations. They may
explain tool selection, but may not write authorization rules, promise access,
or reimplement retrieval/ranking.

Suggested commands:

- `summarize-area`
- `find-evidence`
- `draft-memory`
- `review-proposal`
- `inspect-policy`

## One connector

Select one high-value connector. Prefer a read-heavy connector first. The
connector must support:

- stable external IDs;
- tenant-isolated credentials;
- OAuth or explicit secret lifecycle;
- discovery and permission mapping;
- incremental sync and durable cursors;
- rate limits, timeout, retry, and backoff;
- deletion and revocation semantics;
- Source version creation without duplication;
- connector argument hashing without secrets;
- post-tool quarantine/redaction;
- replay-safe writes where writes are enabled.

Background sync belongs in the worker, not in an interactive MCP request.

## Required tests

### MCP protocol tests

- initialize and capability negotiation;
- `tools/list`, `tools/call`, `resources/list`, `resources/read`;
- prompt listing and retrieval if prompts are exposed;
- malformed JSON-RPC and unknown method errors;
- structured application error mapping;
- cancellation and timeout behavior;
- deterministic tool schemas and ordering;
- stdio process startup, shutdown, and clean stderr behavior;
- no credentials or sensitive arguments in protocol logs.

### Shared enforcement tests

- MCP Rule bypass attempt;
- HTTP Rule bypass attempt;
- connector Rule bypass attempt;
- unauthorized Area retrieval;
- wrong-tenant request;
- wrong actor/agent identity;
- stale authorization context;
- stale Rule version;
- locked global Rule cannot be weakened;
- conflict fails closed and creates Review work;
- approval required before tool execution;
- wrong approver;
- approval replay;
- blocked action never reaches connector;
- post-tool quarantine and sensitive-field redaction;
- argument hashes exclude secrets;
- durable decision records contain Rule ID, version, rationale, purpose,
  actor, outcome, and policy-envelope version.

### Session contract tests

- two hosts retrieve the same reviewed Memory;
- startup recall is Area- and purpose-scoped;
- session-end capture creates Proposals only;
- session-end capture cannot silently activate Memory;
- proposal evidence retains exact Source/chunk references;
- tool results are attributable to actor, agent, session, and connector;
- prompt and skill versions are recorded in the decision envelope.

### Connector tests

- stable external ID mapping;
- duplicate sync is idempotent;
- changed content creates a new Source version;
- deleted/revoked external object is handled correctly;
- external permissions narrow ENGRAVE visibility;
- private content does not cross Areas;
- cursor replay is safe;
- rate-limit retry is bounded;
- timeout/cancellation leaves durable operation state;
- credentials are tenant-isolated and never passed to another connector;
- connector output is quarantined before model disclosure.

### Registry/release tests

- `server.json` validates against the current official schema;
- server name matches package verification metadata;
- package version and registry version match;
- package contains no secrets;
- stdio launch works from a clean machine environment;
- declared environment variables are complete and classified as secret/nonsecret;
- release artifact checksum is reproducible;
- registry metadata is generated from the release;
- publication is performed only from a tagged release;
- a published metadata record points to the intended repository and artifact.

## Acceptance gate

- [ ] Two different agent hosts retrieve the same reviewed Memory.
- [ ] Session-end capture creates Proposals only.
- [ ] MCP tools cannot bypass Rules or Areas.
- [ ] One connector syncs external objects into versioned Sources without
      duplication.
- [ ] Connector credentials are isolated per tenant and connector.
- [ ] Registry metadata is reproducible from a release.
- [ ] Local stdio works without a language runtime prerequisite.
- [ ] The MCP surface remains small enough for reliable tool selection.
- [ ] Exact verification commands and evidence are recorded.

## Explicit non-goals

- Do not start aggressive search or Phase I.
- Do not add dozens of narrowly named tools.
- Do not let prompt text act as security policy.
- Do not publish a remote MCP endpoint before OAuth, tenant isolation, audit,
  rate limits, and revocation are proven.
- Do not claim production readiness from local stdio tests alone.
