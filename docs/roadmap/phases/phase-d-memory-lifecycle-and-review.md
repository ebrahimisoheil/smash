# Phase D — Memory Lifecycle and Review

> Source: SMASH_V2.md §20, Phase D

## Goal

Governed Memory exists: nothing durable becomes active without review or explicit admission policy, and every change is explainable.

## Scope

Implement:

- proposal creation;
- duplicate candidates;
- contradiction candidates;
- review actions;
- editing;
- rejection reasons;
- evidence attachment;
- activation;
- archive and restore;
- expiry;
- review-after;
- supersession.

Build the **Review UI** and the **universal saved-record surface**.

Port V1 lifecycle fixtures and behavioral tests.

Long-running agents may produce checkpoints, evidence, and selective Proposals
while a process runs. They do not write every observation into Memory. A
personal-Area ontology or Memory change can become durable only through the
confirmed admission policy; shared and Cross-Map changes retain their stricter
approval routes.

## Acceptance criteria

- [ ] Agents and ingestion can propose but **cannot silently activate** Memory.
- [ ] A reviewer sees claim, reason, evidence, scope, applicability, and conflicts.
- [ ] Optimistic concurrency prevents overwriting another review.
- [ ] Supersession produces a complete lineage and excludes the old record from default recall.
- [ ] Expiry and applicability behave deterministically.
- [ ] Every decision is explainable through Activity.
- [ ] A long-running process can create selective Proposals and retained
  evidence without silently activating Memory.

## References

- [08 — Memory write and upsert strategy](../08-memory-write-upsert.md)
- [04 — Domain model §4.9–4.10](../04-domain-model.md#49-memory)
- [14 — Web application §14.4 Review, §14.5 Saved record](../14-web-application.md#144-review)
- [17 — Testing §17.1 Contract tests](../17-testing-evaluation.md#171-contract-tests)
- [23 — Diagrams §24.6](../23-diagrams.md#246-memory-proposal-and-upsert-decision-graph)
