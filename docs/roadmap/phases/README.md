# Phases

> Source: the historical roadmap source §20

Each phase is a **capability gate**, not a time box. A phase opens when the previous phase's invariants and acceptance criteria are met.

| Phase | Name |
|---|---|
| A | [Contract and workspace foundation](phase-a-contract-and-workspace-foundation.md) |
| B | [Docker Compose and canonical persistence](phase-b-compose-and-canonical-persistence.md) |
| C | [Source pipeline and worker reliability](phase-c-source-pipeline-and-worker.md) |
| D | [Memory lifecycle and Review](phase-d-memory-lifecycle-and-review.md) |
| E | [Light search and LanceDB](phase-e-light-search-and-lancedb.md) |
| F | [Maps, graph, and Cross-Map](phase-f-maps-graph-and-cross-map.md) |
| G | [Rules and harnesses](phase-g-rules-and-harnesses.md) |
| H | [MCP, skills, prompts, and one connector](phase-h-mcp-skills-prompts-connector.md) |
| I | [Aggressive search](phase-i-aggressive-search.md) |
| J | [Community Edition release gate](phase-j-community-edition-release-gate.md) |

Overview and dependency graph: [../19-phases.md](../19-phases.md).

## Document shape

Each phase document carries:

- **Goal** — the capability the phase adds
- **Scope** — what gets built
- **Acceptance criteria** — the gate; all must hold before the next phase opens
- **References** — the roadmap sections that define the contract being implemented
