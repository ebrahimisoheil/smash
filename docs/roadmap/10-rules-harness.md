# 10 — Rules and Harness Enforcement

> Source: the historical roadmap source §11

Prompts can tell an agent how to behave, but the agent can misunderstand or ignore them. **Rules must be enforced around the agent's actions.**

## Evaluation points

The initial Rule engine evaluates structured context against declarative conditions at these points:

| Point | Capability |
|---|---|
| Before retrieval | Restrict Areas, types, fields, and Source classes |
| After retrieval | Redact or transform fields based on recipient and purpose |
| Before tool call | Allow, warn, require approval, or block |
| After tool result | Quarantine or redact unsafe output |
| Before proposal | Validate target Area, evidence, and sensitivity |
| Before durable write | Enforce review and admission policy |
| At session end | Permit proposal capture while preventing silent activation |

## Decision output

Every decision returns **Rule ID, version, effect, rationale, and next action**.

- A **block** is mechanical: the tool or write does not execute.
- A **warning** is surfaced to both agent and user.
- An **approval** creates a durable decision linked to the eventual operation.

## Priority and scope

- Environment Rules apply broadly.
- Area Rules can **strengthen** them.
- Connector and tool Rules constrain particular integrations.
- **Locked restrictions cannot be loosened by a narrower scope.**
- Conflicts **fail closed** and enter Review.

## Rule test harness

Provide a **Rule test harness before a visual rule builder**.

- Each Rule includes positive, negative, and boundary fixtures.
- Changes run against historical Activity where possible, to show what would have been allowed or blocked.

## Related

- Harness sequence: [23 — Diagrams §24.9](23-diagrams.md#249-rule-harness-around-agent-actions)
- Rule object definition: [04 — Domain model §4.11](04-domain-model.md#411-rule)
- Phase gate: [Phase G — Rules and harnesses](phases/phase-g-rules-and-harnesses.md)
