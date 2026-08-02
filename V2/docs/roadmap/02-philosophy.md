# 02 — Philosophy

> Source: SMASH_V2.md §3

Six principles. Each one constrains implementation, not just marketing.

## 2.1 Storage is not memory

Distinct objects with distinct meanings:

| Object | Definition |
|---|---|
| Source | Evidence |
| Chunk | An addressable part of evidence |
| Embedding | A mathematical representation |
| Entity | An identity |
| Graph edge | A relationship |
| **Memory** | **A governed claim** |

Conflating these creates untrustworthy systems:

- If every retrieved chunk is treated as Memory, the agent receives noise.
- If every extracted statement is stored automatically, the store accumulates duplicates, speculation, obsolete facts, prompt echoes, and contradictions.
- If embeddings become the only representation, users cannot inspect or migrate their data.
- If the graph becomes canonical truth, extraction errors become structural errors.

Boundaries SMASH must preserve:

- Original source bytes remain immutable evidence.
- Extracted text and chunks are derived representations.
- Entities and relationships are structured interpretations.
- Memory Proposals are candidates, not truth.
- Active Memory is admitted through review or an explicit admission policy.
- Vector indexes and caches are rebuildable and never canonical.
- Rules are evaluated mechanically outside the language model.

## 2.2 Humans define meaning; agents can propose structure

Agents may propose an Area, a Map kind, a relation, a Memory, a Rule, or a Cross-Map mapping. They must not silently redefine the system's meaning.

This is a control boundary, not a philosophical objection to automation. When an agent creates a new kind called `Champion`, merges it with `Stakeholder`, or declares that a private call supports a public marketing claim, it changes future retrieval and behavior. Those changes must be **visible, attributable, reversible, and reviewable**.

Automation can become more permissive over time through explicit policies — a team may allow low-risk duplicate evidence to merge automatically, or permit an approved connector to refresh a known field. **Automation is granted by policy, not inferred by the model.**

## 2.3 The reason is part of the record

Every durable Memory explains why it exists.

"Enterprise trials remain 21 days" is incomplete. A trustworthy record also:

- says that security and procurement require the third week;
- identifies the calls and reports that support the decision;
- describes when the decision applies;
- defines what change would trigger review.

The reason improves trust, review quality, retrieval, and future supersession. It lets a new employee or agent distinguish a deliberate decision from a copied sentence. It also makes aggressive search explainable: SMASH can return not only what it found, but why the team previously considered it worth remembering.

## 2.4 Forgetting is a feature

Human memory is useful partly because it is selective. Agent memory should also expire, decay, become stale, be contradicted, and be superseded.

Deleting history is not the default. SMASH separates **what is active** from **what is historically preserved**:

- Temporary context receives an expiry date.
- Time-sensitive facts receive a review date.
- Replaced decisions retain lineage.
- Invalidated claims remain available for audit and historical reconstruction but are excluded from default retrieval.
- Hard deletion is reserved for explicit privacy, retention, or user requests.

## 2.5 Local ownership and managed convenience share one contract

The Community Edition must be genuinely useful, not a demo whose essential features require the cloud. A team must be able to run SMASH with Docker Compose, own its PostgreSQL database and MinIO objects, use local or configured models, and connect agents through MCP.

The managed service sells **operational value**: identity, collaboration, SSO, connectors, scaling, managed workers, backups, observability, policy administration, compliance, and support. It must not require a different Memory model.

Exporting from managed SMASH to Community Edition preserves Sources, Memory, Maps, Rules, events, and stable identifiers wherever policy permits.

## 2.6 Bounded context beats corpus dumping

The normal agent path never loads the whole Memory or Source corpus.

- Agents begin with a small task brief and retrieve more at the moment of need.
- Results include follow-up options rather than exhaustive data.
- The graph opens to a bounded neighborhood, not a global hairball.
- Light search serves most turns. Aggressive search is deliberate and traced.
