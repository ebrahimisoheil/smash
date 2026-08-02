# 08 — Memory Write and Upsert Strategy

> Source: SMASH_V2.md §9

Every agent run needs a deterministic write strategy. **"Upsert" must not mean overwriting whichever semantically similar row appears first.**

## The ordered write pipeline

| # | Step | Question it answers |
|---|---|---|
| 1 | **Authorization** | May this actor propose or directly perform this action in the target Area? |
| 2 | **Rule evaluation** | Does a Rule block, warn, or require approval? |
| 3 | **Normalization** | Produce stable claim text, type, scope, applicability, evidence references, and content hash **without changing meaning**. |
| 4 | **Idempotency** | Has this request or equivalent operation already completed? |
| 5 | **Exact duplicate detection** | Does an active Memory have the same normalized claim and scope? |
| 6 | **Semantic duplicate retrieval** | Which existing memories might express the same claim differently? |
| 7 | **Contradiction detection** | Which active memories appear incompatible in the same applicability context? |
| 8 | **Evidence merge decision** | Should new evidence attach to an existing logical Memory without changing its claim? |
| 9 | **Proposal creation** | Create a reviewable candidate when admission is not explicitly authorized. |
| 10 | **Transactional commit** | Write the new version, links, and Event atomically. |
| 11 | **Projection** | Enqueue lexical and vector index refresh. |

Decision graph: [23 — Diagrams §24.6](23-diagrams.md#246-memory-proposal-and-upsert-decision-graph).

## Rules of the pipeline

**Exact duplicates do not create new logical Memory.** New evidence creates either a new version or an evidence-attachment event, depending on whether the approved claim changes.

**Semantic duplicates require review** unless a strict deterministic rule applies.

Review may be completed conversationally or in the UI through the shared
admission operation. Conversational confirmation is required for forgetting,
material supersession, sensitive or consequential Memory, and other durable
changes whose impact the user should see before activation.

**Contradictions must never coexist silently as equally active truth.** They create a conflict Review item with side-by-side evidence and applicability.

## Supersession

Supersession is the normal resolution for changed decisions or facts:

1. The new Memory version points to the previous logical Memory or version.
2. The previous record becomes `superseded`.
3. The operation stores a human-readable reason.

Historical queries can reconstruct what was active at a past time.

## Optimistic concurrency

Optimistic concurrency protects review and editing. A client submits the version it reviewed. If another actor changed the record, the mutation **fails with the new state** and requires reconciliation rather than overwriting it.
