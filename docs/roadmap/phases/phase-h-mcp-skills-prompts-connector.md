# Phase H — MCP, Skills, Prompts, and One Connector

> Source: the historical roadmap source §20, Phase H

## Goal

Agents of different vendors use the same governed Memory through a small, safe surface — and one external system syncs reliably.

## Scope

### MCP adapter

Implement the slim MCP adapter over the application core:

- local stdio distribution;
- status / recall / proposal / review / ingest / Rules flows;
- structured errors;
- agent-session attribution.

Preserve the official Registry identity and release metadata.

### Skills and prompts

Build versioned skills and prompts for the session loop.

### One connector

Add **one** connector that proves stable external IDs, incremental sync, permission handling, and Source versioning.

Use MCP ingestion where it is sufficient, **but do not force background connector behavior into an interactive protocol**.

## Acceptance criteria

- [x] Two different agent identities/hosts retrieve the same reviewed Memory
      through the live MCP/PostgreSQL proof.
- [x] Session-end capture creates **Proposals only**.
- [x] MCP tools cannot bypass authorization or Rules, including post-retrieval
      disclosure and connector boundaries.
- [x] Connector updates create Source versions without duplication.
- [x] Registry metadata is reproducible from the release version generator.
- [x] The MCP surface remains six stable tools with deterministic schemas.

## Completion evidence

Phase H is complete locally. See [`docs/phase-h-ledger.md`](../../phase-h-ledger.md)
for commands and evidence. The live PostgreSQL suites pass when run against a
disposable migrated database. Streamable HTTP/OAuth, Registry publication, and
release artifact verification remain explicitly deferred; this is not a
production-readiness claim.

## References

- [12 — MCP, skills, prompts, and connectors](../12-mcp-skills-prompts-connectors.md)
- [11 — Agent session contract](../11-agent-session-contract.md)
- [23 — Diagrams §24.11](../23-diagrams.md#2411-mcp-server-consumer-skills-prompts-and-registry)
