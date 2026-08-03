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

- [ ] Two different agent hosts retrieve the same reviewed Memory.
- [ ] Session-end capture creates **Proposals only**.
- [ ] MCP tools cannot bypass authorization or Rules.
- [ ] Connector updates create Source versions without duplication.
- [ ] Registry metadata is reproducible from a release.
- [ ] The MCP surface remains small enough for reliable tool selection.

## References

- [12 — MCP, skills, prompts, and connectors](../12-mcp-skills-prompts-connectors.md)
- [11 — Agent session contract](../11-agent-session-contract.md)
- [23 — Diagrams §24.11](../23-diagrams.md#2411-mcp-server-consumer-skills-prompts-and-registry)
