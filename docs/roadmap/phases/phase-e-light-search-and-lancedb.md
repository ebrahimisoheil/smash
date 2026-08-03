# Phase E — Light Search and LanceDB

> Source: the historical roadmap source §20, Phase E

## Goal

The default retrieval reflex works, is bounded and explainable, is secure by prefilter, and degrades visibly rather than failing.

## Scope

Implement:

- PostgreSQL lexical retrieval;
- tenant-scoped LanceDB Memory and chunk projections;
- embedding jobs;
- namespace resolution;
- security prefilters;
- reconciliation and rebuild;
- the Light search router;
- ranking signals;
- bounded packets;
- degraded lexical-only mode.

### Benchmarks before tuning

Port V1 retrieval benchmarks **before** tuning. Compare lexical, semantic, and blended strategies on claim-shaped and Source-shaped data. Preserve honest confidence labels.

The search contract and mathematical baseline are documented in
[V2 retrieval algorithm](../../contracts/retrieval-algorithm.md) and
[V2 retrieval mathematics](../../contracts/retrieval-math.md). The legacy
`memory-engrave` benchmark profile is a reproduction target only; its numbers do
not become V2 evidence until the same profile is rerun with V2 identity,
authorization, lifecycle, and fixture rules.

## Acceptance criteria

- [ ] The fixture is retrievable through clean natural-language paraphrases.
- [ ] Unauthorized and wrong-Area records **never appear in candidates returned to the agent**.
- [ ] Index deletion or corruption can be repaired from PostgreSQL and MinIO.
- [ ] An Enterprise Admin can retrieve across permitted tenant Areas while a Normal User cannot escape assigned Areas.
- [ ] Lexical-only mode remains useful and visible.
- [ ] Retrieval packets include reasons, provenance, applicability, and token estimates.
- [ ] **Benchmark regressions block release.**

## References

- [09 — Retrieval architecture §9.1–9.2, §9.5](../09-retrieval-architecture.md)
- [05 — Storage §5.3 LanceDB](../05-storage-responsibilities.md#53-lancedb-is-a-rebuildable-retrieval-sidecar)
- [15 — Auth and security](../15-auth-security.md)
- [17 — Testing §17.3 Retrieval tests](../17-testing-evaluation.md#173-retrieval-tests)
- [23 — Diagrams §24.4, §24.7](../23-diagrams.md#244-canonical-data-and-projection-relationship)
