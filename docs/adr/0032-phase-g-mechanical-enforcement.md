# ADR-0032: Phase G mechanical enforcement boundaries

## Status

Accepted for Phase G.

## Decision

HTTP search invokes the shared preflight before constructing retrieval input,
narrows the Area set through `apply_policy_envelope`, then gives the same
narrowed authorization to lexical and dense paths. `PreToolGateway` is the
reusable before-tool boundary for MCP and native connectors; it returns block
and approval decisions outside the model. Rule decisions can be durably
recorded without recording tool arguments or credentials.

The MCP binary uses the same `RuleEvaluator` path rather than a no-op
authorization implementation. Existing retrieval eligibility still filters
tenant, Area, visibility, lifecycle, purpose, and private ownership before
ranking; Cross-Map expansion retains its approved mapping checks.

## Evidence

The API reconstructs the evaluator from tenant-scoped active PostgreSQL Rule
versions at the retrieval boundary. The core killer-path test proves one
locked private-content block is applied before retrieval, before disclosure,
and before a native/MCP-style tool call. The live Rule test proves activation
idempotency, active-version loading, and durable decision recording; the live
API test proves HTTP search is blocked before retrieval and the blocked
decision is durably recorded.

This is Phase G evidence only; it is not a production-readiness claim.
