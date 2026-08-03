# ENGRAVE V2 Documentation

| Area | Location | Purpose |
|---|---|---|
| Roadmap | [`roadmap/`](roadmap/README.md) | Product philosophy, domain model, architecture, phases, diagrams — the implementation source of truth |

Additional documentation directories (API reference, operator guides, contributor guides) are created as the corresponding phases in [`roadmap/phases/`](roadmap/phases/README.md) land. Until then, the roadmap is the contract.

## Rules for these docs

- The roadmap is more durable than a backlog. A backlog answers what the team will do next; the roadmap answers what the product means and what must remain true as its implementation changes.
- No delivery dates, sprint estimates, or file-by-file coding instructions.
- Architecture decisions are recorded before code depends on them. A normative decision changes only through an explicit architecture decision record.
- Diagrams in [`roadmap/23-diagrams.md`](roadmap/23-diagrams.md) are part of the implementation contract. Update them when a service boundary or data responsibility changes.
