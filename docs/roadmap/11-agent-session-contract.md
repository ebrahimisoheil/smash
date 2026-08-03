# 11 — Agent Session Contract

> Source: the historical roadmap source §12

ENGRAVE defines **one portable agent loop, independent of host application**.

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
