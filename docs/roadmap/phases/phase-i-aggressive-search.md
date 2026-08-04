# Phase I — Aggressive Search

> Source: the historical roadmap source §20, Phase I

## Goal

Deliberate investigation that is budgeted, traced, cited, and incapable of silently changing Memory.

## Scope

Implement:

- deliberate multi-step retrieval;
- query decomposition;
- Cross-Map expansion;
- Source inspection;
- graph traversal;
- contradiction checks;
- reranking;
- progress;
- trace.

Add **budgets** for iterations, Sources, Areas, model calls, tokens, and elapsed work.

Aggressive search may use configured models, but it must preserve citations and **distinguish retrieved evidence from synthesized conclusions**.

It creates optional Proposals, **never active Memory**.

## Acceptance criteria

- [ ] Users can see what is happening and **stop** an investigation.
- [ ] Every final claim links to evidence or is labeled uncertain.
- [ ] Search traces show Areas, mappings, Sources, and ranking stages.
- [ ] Budgets prevent runaway recursion and cost.
- [ ] Contradiction cases improve over Light search on the evaluation set.
- [ ] **No aggressive-search result changes Memory without review.**

## Carried gaps and release boundary

The bounded implementation and contract-level evidence are complete locally.
The following items remain explicit follow-up work and must not be silently
converted into a production-readiness claim:

- A corpus-level Light-vs-Aggressive evaluation is still required. The current
  deterministic fixture proves contradiction exposure for one opposing-claim
  case, not general retrieval quality or a benchmark-wide improvement.
- A live worker test must mutate active authorization between stages and prove
  that a later retrieval, traversal, Source inspection, connector call, or
  disclosure observes the narrowed Rule state.
- The live worker pipeline should include an approved Cross-Map mapping and a
  graph fixture, so `Traverse` is proven in the worker path rather than only by
  core/storage graph tests.
- Connector-backed external calls, OAuth, remote MCP, Registry publication,
  artifact verification, and release security review remain outside Phase I.

Phase J may consume these as release-gate inputs, but it must first preserve
the distinction between locally verified implementation, deferred evidence,
and production/release readiness.

## References

- [09 — Retrieval architecture §9.3 Aggressive search](../09-retrieval-architecture.md#93-aggressive-search)
- [14 — Web application §14.6 Search](../14-web-application.md#146-search)
- [16 — Observability](../16-observability-operations.md)
- [23 — Diagrams §24.7](../23-diagrams.md#247-light-and-aggressive-retrieval-router)
