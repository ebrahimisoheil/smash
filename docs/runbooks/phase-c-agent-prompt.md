# Phase C Agent Handoff Prompt

You are the Phase C implementation agent for ENGRAVE V2.

## Branch contract

Work only on the branch `v2-publish-cxcy`. Confirm the current branch before
editing. Do not implement Phase C work directly on `master`, and do not merge
or delete branches unless explicitly instructed.

## Required vault reads

Use Obsidian MCP and read these notes in order:

1. `ENGRAVE V2/Plans/Phase B Progress.md`
2. `ENGRAVE V2/Phases/Phase C.md`
3. `ENGRAVE V2/Roadmap/06 Service Architecture.md`
4. `ENGRAVE V2/Roadmap/04 Domain Model.md`
5. `ENGRAVE V2/Roadmap/07 Source Ingestion.md`
6. `ENGRAVE V2/Roadmap/13 API Principles.md`
7. `ENGRAVE V2/Roadmap/15 Auth and Security.md`
8. `ENGRAVE V2/Roadmap/16 Observability and Operations.md`
9. `ENGRAVE V2/Roadmap/17 Testing and Evaluation.md`

For long-running-agent behavior, also read:

10. `ENGRAVE V2/Roadmap/10 Rules and Harness.md`
11. `ENGRAVE V2/Roadmap/11 Agent Session Contract.md`
12. `ENGRAVE V2/Roadmap/12 MCP Skills Prompts Connectors.md`

## Scope

Implement and prove:

- durable Operations/jobs;
- worker claiming, leases, retry, and idempotency;
- progress, cancellation, and checkpoint/resume;
- Source ingestion state transitions;
- artifacts and exact-coordinate Chunks;
- processor lineage and reprocessing;
- quarantine for unsafe or unreadable inputs;
- honest API/UI processing states;
- long-running agent process evidence without silent Memory activation.

The agent may retain checkpoints, evidence, and intermediate artifacts. It must
not convert every observation into Memory. Personal-Area Memory, Map, and Rule
changes follow the confirmed admission policy; shared and Cross-Map changes
retain their stricter approval routes.

Do not expand Phase C into retrieval, Map execution, Rule execution, MCP
implementation, billing, or hosted deployment.

## Working protocol

Before coding, produce a bounded session plan and identify the exact files to
change. Preserve existing user changes and never stage unrelated workspace
artifacts.

After each session, update the Phase C Obsidian ledger with:

- status;
- decisions that later agents must not re-derive;
- open questions and owners;
- exact verification commands and evidence;
- commit reference.

Do not mark Phase C complete without proving retry safety, lease recovery,
cancellation, quarantine, exact Source coordinates, checkpoint resume, and no
silent durable Memory activation.

Commit the work on `v2-publish-cxcy`. Push only when explicitly requested.
