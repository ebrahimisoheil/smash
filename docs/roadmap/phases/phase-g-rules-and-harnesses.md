# Phase G — Rules and Harnesses

> Source: the historical roadmap source §20, Phase G

## Goal

Policy is enforced mechanically, outside the model.

## Scope

Implement:

- the declarative Rule model;
- priority and scope;
- evaluation points;
- `allow` / `warn` / `require_approval` / `block` effects;
- decision records;
- test fixtures.

Integrate Rules into **retrieval, Source disclosure, writes, and external tool calls**.

### Pre-tool gateway

Create a pre-tool gateway that host integrations, MCP consumers, and native connectors can use.

**Do not rely on the model to call the gate voluntarily when the host supports mechanical interception.**

## Acceptance criteria

- [ ] A block prevents the controlled action **outside the model**.
- [ ] Every decision names the Rule version and rationale.
- [ ] Global locked restrictions cannot be weakened by Area Rules.
- [ ] Conflicts fail closed and create Review work.
- [ ] Rule tests run before activation.
- [ ] **The killer demo can block publication of private Source evidence.**

## References

- [10 — Rules and harness enforcement](../10-rules-harness.md)
- [04 — Domain model §4.11 Rule](../04-domain-model.md#411-rule)
- [15 — Auth and security](../15-auth-security.md)
- [17 — Testing §17.4 Security tests, §17.5 End-to-end tests](../17-testing-evaluation.md#174-security-tests)
- [23 — Diagrams §24.9](../23-diagrams.md#249-rule-harness-around-agent-actions)
