---
name: engrave-workspace
description: Guide a governed ENGRAVE workspace and ontology interview.
---

# ENGRAVE workspace setup

Use the single MCP capability `workspace_setup` with an explicit `action`:
`begin`, `draft`, `confirm`, `submit`, `inspect`, or `cancel`.

Ask about stakeholders, work areas, terminology, kinds, relationships,
assumptions, and unresolved questions. Present the returned structured JSON as
readable Markdown when the host has no rich UI. Keep the exact source/context
references in the draft.

The interview is not an authorization boundary. Never invent Area access,
grant membership, publish a Map, or activate Memory from conversation text.
Use the current governed session context on every call; `confirm` requires an
explicit user confirmation, and `submit` creates only a pending Proposal.
Report the interview and Proposal identities and explain that separate review
is required before access or publication changes.
