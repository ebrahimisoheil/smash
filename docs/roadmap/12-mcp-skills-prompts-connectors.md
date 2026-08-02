# 12 — MCP, Skills, Prompts, and Connectors

> Source: SMASH_V2.md §13

## 12.1 MCP server

SMASH exposes governed Memory through MCP. **The model-facing surface stays small.**

V1's slim pattern is worth preserving: `status`, `recall`, `remember/propose`, `ingest`, `review`, `Rules`, and an administration escape hatch. Avoid exposing dozens of narrowly named tools — they increase selection errors and consume context.

| Deployment | Transport |
|---|---|
| Community Edition (local) | stdio for desktop agents; optional loopback Streamable HTTP for compatible local hosts |
| Managed | Streamable HTTP with OAuth-compatible authorization, audience-bound tokens, explicit scopes, protected-resource metadata |

MCP responses use the **same core-crate application contracts and authorization** as the Axum API. The MCP adapter is a crate in the same Rust workspace — an **adapter, not a separate implementation of Memory logic**. Tool errors are structured, safe, and actionable, using the same application error type as the HTTP surface.

Shipping the MCP server as a compiled Rust binary means the local Community Edition install is a single self-contained executable with no language runtime prerequisite on the user's machine.

## 12.2 Agent session hooks and selective writes

The user experience is an agent loop, not a database interaction. The local
MCP installation should make recall feel automatic while keeping durable writes
rare, explainable, and governed.

### Pre-hook: quiet recall before the model answers

On each eligible user turn, the host skill invokes a non-mutating pre-hook:

1. classify the request enough to choose no recall, targeted recall, or a brief;
2. retrieve reviewed, tenant-authorized Memory and relevant Source evidence;
3. attach a compact, provenance-bearing context packet to the agent turn;
4. record the retrieval/run identifiers without changing canonical Memory.

The pre-hook must be fast, bounded, and safe to skip when the turn is clearly
casual or unrelated. It never proposes or writes Memory.

### Post-hook: capture candidates, not every turn

The post-hook is not a transcript sink. It runs at session end, on an explicit
“remember/save this” instruction, or when a strong durable-signal policy fires:
new stable fact, user correction, decision, commitment, preference, or
reusable workflow. It must suppress greetings, brainstorming, transient plans,
duplicated context, and ordinary answers.

Post-hook output is a **Proposal** with evidence, confidence, novelty/conflict
classification, and a reason. It does not write active Memory automatically;
review or an explicitly authorized write path is required. A rejected or
deferred candidate is still auditable, but it must not reappear on every turn.

The hook contract is host-independent: skills orchestrate timing, MCP exposes
the governed operations, and core owns retrieval, deduplication, authorization,
Rules, and write policy. This separation keeps 90% of the experience in the
agent while keeping durable state outside the model.

## 12.3 MCP consumer and connector gateway

SMASH also **consumes** approved MCP servers. Their resources can become Sources; their tools can be invoked through the Rule gateway.

**External tool descriptions and content are untrusted.** Installation requires explicit trust metadata, scopes, and administrator approval where applicable.

The gateway records: server identity, version, tool, arguments hash, actor, Rule decision, approval, result classification, and resulting Sources or Events.

**Tokens received for one server are never passed to another.**

## 12.4 MCP Registry and GitHub.io release plan

Registry publication is a **release concern**, never a runtime dependency for
recall, proposal, or write behavior.

Before Community Edition release:

- validate the official identity `io.github.ebrahimisoheil/smash`;
- generate and review `server.json` and package metadata from the exact release;
- publish the GitHub.io installation/release page with supported hosts,
  platform binaries, checksums, permissions, and upgrade instructions;
- publish Registry metadata only after the binaries and package artifacts exist;
- run a clean-install smoke test from the GitHub.io instructions and verify
  install → pre-recall → answer → selective proposal/write;
- record the published version and provide a rollback/unpublish procedure.

The Registry hosts **metadata, not application artifacts**. Distribution remains
the responsibility of crates.io, prebuilt binaries, containers, and the
GitHub.io release surface. The Registry is a discovery channel, not a trust
authority; local installation still applies explicit trust, scopes, Rules, and
revocation.

When the managed MCP endpoint is production-ready, add a remote server declaration with the correct transport and authorization metadata. **Do not publish a remote endpoint before tenant isolation, OAuth, rate limits, audit, and revocation are complete.**

The official Registry is a **discovery channel, not a trust authority**. SMASH maintains its own trusted connector catalog for installed servers, publisher verification, requested scopes, permitted tools, Rules, security notices, and revocation.

## 12.5 Skills

Skills teach agents the SMASH session contract and domain-specific workflows. **Keep them thin.**

A skill must not reimplement ranking, write policy, or authorization. It invokes MCP or HTTP APIs and explains when to use them.

Initial skills:

- startup recall;
- targeted retrieval;
- Source ingestion;
- review;
- session-end proposal capture;
- health diagnostics;
- Sales memory;
- connector setup.

Each skill needs versioning, compatibility metadata, test prompts, and a clear description of mutations.

## 12.6 Prompts

Prompts are user- or agent-invoked templates for consistent workflows. **They are not Rules.**

Useful prompts:

- start with SMASH;
- create a brief;
- investigate aggressively;
- propose a Memory;
- review evidence;
- resolve a contradiction;
- ingest a Source;
- close a session.

Prompt versions are recorded when they generate Proposals. Prompt text belongs in version-controlled assets and can be overridden deliberately.

**Do not store secret policies only in prompts.**

## 12.7 Native connectors

A connector turns an external system into stable Source objects and, optionally, controlled actions.

Every connector implements:

- authorization;
- discovery;
- incremental sync;
- stable external IDs;
- cursors;
- rate-limit handling;
- deletion semantics;
- permission mapping;
- webhook or polling behavior.

A connector is a **trait implementation in the core crate**, driven by the worker. Outbound HTTP uses **Reqwest** with explicit timeouts, retry and backoff policy, and connection reuse. Credentials are resolved per call from encrypted storage — never captured in a long-lived client shared across tenants.

**Start with one high-value connector plus direct uploads**, not a broad catalog. A Notion, CRM, or meeting connector is selected based on design-partner workflow.

MCP-based access can accelerate coverage, but native connectors remain valuable when background synchronization, webhooks, file downloads, or service accounts are required.

## Topology

See [23 — Diagrams §24.11](23-diagrams.md#2411-mcp-server-consumer-skills-prompts-and-registry).
