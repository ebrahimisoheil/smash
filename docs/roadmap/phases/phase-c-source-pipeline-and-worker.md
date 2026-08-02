# Phase C — Source Pipeline and Worker Reliability

> Source: SMASH_V2.md §20, Phase C

## Goal

Evidence enters the system reliably, is addressable, and never silently becomes truth.

## Scope

### Job system

Implement:

- the Operation/job system;
- worker claiming;
- leases;
- retry policy;
- progress;
- cancellation semantics.

The same infrastructure supports long-running agent processes: durable
checkpoints, safe retry and lease reclamation, progress updates, cancellation,
and explicit failure state. The worker may retain process evidence and
intermediate artifacts, but durable Memory activation belongs to later
governance phases.

### Narrow, dependable Source set first

- text / Markdown;
- PDF;
- common images with OCR or description;
- **one** structured or audio format, based on available processing.

### Artifacts and chunks

Create stable artifacts and chunks, exact evidence coordinates, processor lineage, and reprocessing.

Surface processing state in API and UI. **Quarantine unsafe or unreadable inputs.**

## Acceptance criteria

- [ ] Every Source reaches an honest terminal or actionable state.
- [ ] Retrying a completed or failed job does not duplicate canonical results.
- [ ] Chunks resolve back to exact Source coordinates.
- [ ] Changing a processor produces a new derived version **without changing original bytes**.
- [ ] Parser failures and suspicious files are visible and recoverable.
- [ ] **No processor activates durable Memory.**
- [ ] Long-running work resumes from a durable checkpoint and never silently
  converts execution observations into active Memory.

## References

- [07 — Source ingestion](../07-source-ingestion.md)
- [06 — Service architecture §6.2 Worker](../06-service-architecture.md#62-worker-uses-the-backends-core-crate)
- [04 — Domain model §4.6–4.7](../04-domain-model.md#46-source)
- [23 — Diagrams §24.5](../23-diagrams.md#245-source-ingestion-pipeline)
