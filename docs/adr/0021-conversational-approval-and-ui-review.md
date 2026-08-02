# ADR-0021: Conversational approval and UI review are one admission contract

## Status

Accepted

## Context

The UI cannot be the only place where a user approves important Memory
mutations. A user may be chatting through an MCP host when the agent discovers
a durable preference, a correction, a request to forget, or a new Rule/Harness.
Forcing that person to leave the conversation creates friction and encourages
silent or ambiguous writes.

## Decision

SMASH exposes two approval routes over the same governed admission operation:

1. **Conversational confirmation** for high-impact or clearly user-requested
   mutations. The agent summarizes the proposed change, scope, evidence,
   permanence, and consequences, then asks for explicit confirmation.
2. **UI review** for deferred, batched, or complex review. The user can edit,
   compare provenance, test a Rule, approve, reject, or postpone the Proposal.

Both routes create the same durable decision envelope and re-check the Rule,
scope, actor, and version immediately before mutation. A conversational “yes”
is not a bypass around authorization or the Rule harness.

The agent must ask before:

- activating or strengthening a personal or Area Rule/Harness;
- forgetting, deleting, suppressing, or materially superseding Memory;
- storing a sensitive, consequential, or long-lived preference or decision;
- performing an external or otherwise irreversible action.

For low-risk memory candidates, the default remains Proposal-only capture with
no interruption. The user may choose “review in UI” instead of confirming in
conversation. No durable mutation is silently activated merely because a
post-hook ran.

## Consequences

- MCP and UI clients share one admission API and audit model.
- The agent can preserve conversational flow while escalating consequential
  changes at the moment they are understood.
- Review UX must show the exact proposal, evidence, affected scope, Rule
  decision, and confirmation channel.
- Tests must cover both approval routes, stale proposals, cancellation, and
  attempts to bypass the harness.
