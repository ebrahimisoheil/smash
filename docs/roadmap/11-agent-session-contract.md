# 11 — Agent Session Contract

> Source: SMASH_V2.md §12

SMASH defines **one portable agent loop, independent of host application**.

## Session start

The host or skill checks status and requests a **small brief** for the task and active Area.

- If an agent host supports hooks, this happens automatically.
- Otherwise a prompt or skill initiates it.

The brief contains **Shared Rules, high-applicability Memory, and review warnings** — not the entire user profile.

## During work

- The agent calls **Light search** before asking the user to repeat durable context or reading broad source collections.
- It requests **Aggressive search** for investigation, verification, and high-impact decisions.
- Before controlled external actions, **the harness evaluates Rules**.

## Session end

The agent or hook may capture observations and generate **Proposals**.

**It must not activate durable Memory without an explicit admission policy.**

Duplicate, echo, and trivial-session guards from V1 are preserved and adapted to the new event model.

## Admission and approval channel

The UI is an important review surface, but it is not the only place where
approval can happen. MCP and UI clients use the same admission operation.

- Low-risk candidates remain **Proposal-only** and do not interrupt the chat.
- High-impact mutations trigger an in-conversation confirmation when the user
  is present: the agent explains the proposed change, scope, evidence,
  permanence, and consequences, then asks for an explicit yes/no decision.
- The user may choose **review in UI** for batch, complex, or deferred review.
- The agent must ask before activating a Rule/Harness, forgetting or materially
  superseding Memory, storing a sensitive or consequential preference/decision,
  or performing an irreversible external action.

Conversational confirmation and UI approval create the same durable decision
envelope and re-check authorization, Rules, scope, and version immediately
before mutation. A post-hook never silently activates a durable mutation.

## Request identity

Each request carries:

- agent identity;
- host identity;
- session ID;
- task;
- active Area;
- idempotency key.

This makes retrieval and writes explainable across Codex, Claude, ChatGPT, Cursor, internal agents, and future hosts.

## State machine

See [23 — Diagrams §24.10](23-diagrams.md#2410-agent-session-and-memory-loop).
