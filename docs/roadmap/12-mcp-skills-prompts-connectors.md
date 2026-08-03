# 12 — MCP, Skills, Prompts, and Connectors

> Source: the historical roadmap source §13

## 12.1 MCP server

ENGRAVE exposes governed Memory through MCP. **The model-facing surface stays small.**

V1's slim pattern is worth preserving: `status`, `recall`, `remember/propose`, `ingest`, `review`, `Rules`, and an administration escape hatch. Avoid exposing dozens of narrowly named tools — they increase selection errors and consume context.

| Deployment | Transport |
|---|---|
| Community Edition (local) | stdio for desktop agents; optional loopback Streamable HTTP for compatible local hosts |
| Managed | Streamable HTTP with OAuth-compatible authorization, audience-bound tokens, explicit scopes, protected-resource metadata |

MCP responses use the **same core-crate application contracts and authorization** as the Axum API. The MCP adapter is a crate in the same Rust workspace — an **adapter, not a separate implementation of Memory logic**. Tool errors are structured, safe, and actionable, using the same application error type as the HTTP surface.

Shipping the MCP server as a compiled Rust binary means the local Community Edition install is a single self-contained executable with no language runtime prerequisite on the user's machine.

## 12.2 MCP consumer and connector gateway

ENGRAVE also **consumes** approved MCP servers. Their resources can become Sources; their tools can be invoked through the Rule gateway.

**External tool descriptions and content are untrusted.** Installation requires explicit trust metadata, scopes, and administrator approval where applicable.

The gateway records: server identity, version, tool, arguments hash, actor, Rule decision, approval, result classification, and resulting Sources or Events.

**Tokens received for one server are never passed to another.**

## 12.3 Official MCP Registry

- Preserve the existing official Registry identity `io.github.ebrahimisoheil/engrave` for Community Edition.
- Maintain `server.json`, package metadata, and automated publishing as release artifacts.
- The Registry hosts **metadata**, not application artifacts — so package publication remains part of distribution: crates.io for the Rust package, plus prebuilt platform binaries and container images for users who do not have a Rust toolchain.

When the managed MCP endpoint is production-ready, add a remote server declaration with the correct transport and authorization metadata. **Do not publish a remote endpoint before tenant isolation, OAuth, rate limits, audit, and revocation are complete.**

The official Registry is a **discovery channel, not a trust authority**. ENGRAVE maintains its own trusted connector catalog for installed servers, publisher verification, requested scopes, permitted tools, Rules, security notices, and revocation.

## 12.4 Skills

Skills teach agents the ENGRAVE session contract and domain-specific workflows. **Keep them thin.**

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

## 12.5 Prompts

Prompts are user- or agent-invoked templates for consistent workflows. **They are not Rules.**

Useful prompts:

- start with ENGRAVE;
- create a brief;
- investigate aggressively;
- propose a Memory;
- review evidence;
- resolve a contradiction;
- ingest a Source;
- close a session.

Prompt versions are recorded when they generate Proposals. Prompt text belongs in version-controlled assets and can be overridden deliberately.

**Do not store secret policies only in prompts.**

## 12.6 Native connectors

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
