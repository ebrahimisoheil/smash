# ADR-0031: Phase G declarative Rule contract

## Status

Accepted for Phase G.

## Decision

Rules are typed data in `engrave-core::rules`, with tenant/environment, Area,
actor/persona/role/agent, purpose/session, object, lifecycle, sensitivity,
field, connector/tool, priority, lock, activation, and effective-time scope.
Only declarative conditions are accepted; arbitrary executable user code is
not a Rule input. `RuleEvaluator` is deterministic, rejects invalid active
configuration, fails closed on conflicting allow/block matches, and emits a
versioned `PolicyEnvelope` plus Rule ID/version, rationale, effect, and next
action.

Locked restrictions are never weakened by narrower matches. The envelope is
context for the model, while `RuleEvaluator` and the caller-side gateway are
the enforcement mechanism.

## Evidence and limitation

Core contract tests cover locked conflict failure, approval next-action and
envelope output. The PostgreSQL migration persists Rule versions, tests,
decisions, approvals, conflicts, and idempotency. A live database run is still
required to record fresh Phase G migration/repository evidence in this
checkout; this ADR does not claim production readiness.
