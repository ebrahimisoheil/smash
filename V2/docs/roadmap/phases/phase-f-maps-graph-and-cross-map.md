# Phase F — Maps, Graph, and Cross-Map

> Source: SMASH_V2.md §20, Phase F

## Goal

Areas have versioned semantic contracts, the graph stays bounded, and cross-Area meaning connects conservatively without leaking permissions.

## Scope

Implement:

- versioned Map kinds and relations;
- entity identity;
- relationship proposals;
- bounded graph queries;
- Cross-Map mappings.

Build Area **Board**, **graph**, and **Map review** experiences.

Cross-Map expansion enters Light search **only through explicit approved mappings and configured limits**.

Add permission and identity-merge **adversarial tests**.

## Acceptance criteria

- [ ] Structured objects retain the Map version that interpreted them.
- [ ] Map changes create reviewable versions and migrations.
- [ ] Graph views remain bounded and searchable.
- [ ] Cross-Map results preserve original labels and mapping paths.
- [ ] Blocked or unauthorized mappings cannot leak candidates.
- [ ] Same-identity merges can be reversed **without losing Area-local records**.

## References

- [04 — Domain model §4.4 Map, §4.5 Cross-Map, §4.8 Entity and relationship](../04-domain-model.md#44-map)
- [09 — Retrieval architecture §9.4 Cross-Map retrieval](../09-retrieval-architecture.md#94-cross-map-retrieval)
- [14 — Web application §14.2 Areas](../14-web-application.md#142-areas)
- [23 — Diagrams §24.8](../23-diagrams.md#248-area-maps-and-cross-map-architecture)
