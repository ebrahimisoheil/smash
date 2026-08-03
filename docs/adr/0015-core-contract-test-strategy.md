# ADR-0015: Core contract test strategy

Status: Accepted  
Date: 2026-08-02

## Decision

Product invariants are tested against `engrave-core` directly. The first contract
tests cover lifecycle legality, duplicate refusal, contradiction handling,
supersession, Rule precedence, event emission, idempotency, and optimistic
concurrency. API, worker, and MCP tests remain thin translation, middleware,
and adapter tests; they do not become alternate policy implementations.

## Consequence

If a rule only holds when called over HTTP, the rule is in the wrong layer.
Ports receive deterministic fakes for clock, IDs, storage, and queue behavior.
