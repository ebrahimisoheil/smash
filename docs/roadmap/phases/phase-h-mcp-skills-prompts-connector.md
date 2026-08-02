# Phase H — MCP, Skills, Prompts, and One Connector

> Source: SMASH_V2.md §20, Phase H

## Goal

Agents of different vendors use the same governed Memory through a small, safe surface — and one external system syncs reliably.

## Scope

### MCP adapter

Implement the slim MCP adapter over the application core:

- local stdio distribution;
- status / recall / proposal / review / ingest / Rules flows;
- structured errors;
- agent-session attribution.

Define and test the session hooks: pre-hook recall is non-mutating and bounded;
post-hook capture is selective, proposal-first, and never a transcript sink.
The hook tests must cover casual turns, explicit save requests, user
corrections, duplicate suppression, conflict proposals, and session-end
capture.

The MCP adapter must also support conversational approval for high-impact
mutations and offer “review in UI” for deferred or complex review. Both routes
must use the same admission operation; neither may bypass Rules or
authorization.

Registry and GitHub.io publication are release work, not runtime dependencies.
Prepare the exact-release `server.json`, package metadata, installation page,
checksums, clean-install smoke test, and rollback procedure for Phase J.

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
- [ ] Pre-hook recall and selective post-hook proposal behavior pass the agent UX tests.
- [ ] Rule/Harness activation, forgetting, and consequential Memory decisions ask for conversational confirmation or explicitly route to UI review.
- [ ] Registry metadata and GitHub.io installation instructions are reproducible from a release.
- [ ] A clean install from GitHub.io completes the full session loop.
- [ ] The MCP surface remains small enough for reliable tool selection.

## References

- [12 — MCP, skills, prompts, and connectors](../12-mcp-skills-prompts-connectors.md)
- [11 — Agent session contract](../11-agent-session-contract.md)
- [23 — Diagrams §24.11](../23-diagrams.md#2411-mcp-server-consumer-skills-prompts-and-registry)
