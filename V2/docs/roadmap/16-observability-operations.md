# 16 — AI Decision Observability and Operations

> Source: SMASH_V2.md §17

SMASH needs **two correlated observability systems** with different purposes and retention.

| System | Purpose | Sampling | Sensitivity |
|---|---|---|---|
| **Operational telemetry** | Explain service behavior: latency, failure, model request, retrieval call, worker execution, tool invocation | May be sampled | Does not capture sensitive prompt or Source content by default |
| **Tenant decision ledger** | Canonical product data: what an agent was trying to accomplish, which exact context it received, why those records were selected, which Rules and approvals applied, what it recommended or executed, what application outcome followed | **Not sampled away** | Content access follows enterprise roles and retention policy |

Operational telemetry uses logs, metrics, and OpenTelemetry-compatible spans, emitted through the Rust `tracing` ecosystem with an OpenTelemetry exporter. Span context is established by a Tower layer so every handler, job, and outbound call inherits it without manual plumbing.

## Structured logging and metrics

Use structured logs with these identifiers: request, tenant, actor, agent, session, AI run, Operation, trace.

Metrics cover:

- API latency;
- worker queue depth and job age;
- failure rates;
- extraction duration;
- embedding duration;
- PostgreSQL query latency;
- MinIO errors;
- LanceDB query latency;
- retrieval candidate counts and result counts;
- token estimates;
- Rule decisions;
- review outcomes.

## 16.1 Decision trace hierarchy

```
tenant → agent session → AI run/task → retrieval/model/rule/tool spans → decision → application outcome → human feedback
```

One session can contain many runs; one run can contain several retrievals and tool calls.

Canonical records cover: AI sessions and runs, decision envelopes, retrieval events/items, model invocations, tool calls, Rule evaluations, human approvals, decisions, outcome links, feedback, snapshot references, access grants, content-access events.

Frequently filtered dimensions belong in **relational columns**. Provider-specific detail may use validated JSONB. **The entire trace must not become one unqueryable JSON document.**

## 16.2 Decision envelope

For every significant recommendation or action, the immutable decision envelope references:

- tenant, user/service actor, agent, host, session, run, task, and active Area;
- requested and resolved model, relevant parameters, and provider;
- Prompt, Skill, Map, Cross-Map, connector, and tool-definition versions;
- exact Memory versions and Source chunks retrieved, with scores, ranks, and selection reasons;
- applicability, contradiction, staleness, and permission evaluations;
- Rule versions, effects, and human approvals;
- tool arguments/results, or protected snapshot references;
- final recommendation/action, evidence, uncertainty, and resulting Proposals;
- application object and later outcome or human correction.

Large, sensitive, or multimodal bodies live as **classified, encrypted MinIO snapshots** with content hashes and retention. PostgreSQL stores the relationships needed to trace and analyze them.

**Trace capture must be configurable by enterprise policy**, because decision records can be more sensitive than ordinary application telemetry.

## 16.3 AI Tracer and Replay

Future **AI Tracer** reconstructs the decision path as a graph of retrievals, Memory influence, model calls, Rules, approvals, tools, and outcomes.

It answers *"what did the AI know when it decided?"* rather than only *"how many tokens did it use?"*

### Three replay modes

| Mode | Behavior |
|---|---|
| **Forensic replay** | Reconstruct the exact recorded context, tools, Rules, and outputs **without executing side effects** |
| **Execution reproduction** | Invoke the same or pinned model configuration, while warning that hosted-model nondeterminism and version drift can prevent identical output |
| **Counterfactual replay** | Substitute current Memory, different Rules, excluded Areas, or another model to compare decisions |

**Replay never repeats external side effects by default.** It uses recorded tool results, mocks, or an explicit sandbox. Every replay is itself a tenant trace linked to the original run.

## 16.4 Application-level AI analytics

The future analytics product connects:

```
Source → Memory → retrieval → AI decision → tool/application action → business outcome → human feedback
```

It answers questions such as:

- which Memory influences successful opportunities;
- which Sources produce rejected Proposals;
- which decisions are repeatedly corrected;
- which Rules prevent unsafe actions;
- where Areas contradict each other;
- which workflows benefit from Aggressive search.

**Consumers:** Enterprise Admin and AI Governance Admin are primary. Normal Users receive only their own or explicitly shared analytics.

**Cross-tenant product analysis uses explicit consent, aggregation, and minimum cohort protections. Raw customer prompts, Sources, and decisions are never silently pooled.**

## Health

Health has layers:

| Layer | Meaning |
|---|---|
| Liveness | Process can respond |
| Readiness | Required dependencies and migrations are usable |
| Subsystem health | PostgreSQL, MinIO, LanceDB, worker state |
| Product health | Stuck Sources, stale indexes, failed Operations, review backlog, event inconsistencies |

Community Edition exposes a **human-readable diagnostics page** and a **machine-readable health API**. Repair actions are explicit and safe.

**A health page must never mutate data just by loading.**

## Backups

Backups include PostgreSQL, MinIO objects, LanceDB (or the ability to rebuild it), configuration, and encryption-key procedures.

**A backup that has never been restored is not a backup.**

Document consistent snapshot ordering and acceptable recovery semantics.

## Related

- Trace/replay/outcome graph: [23 — Diagrams §24.16](23-diagrams.md#2416-ai-decision-trace-replay-and-outcome-graph)
- Decision records in the domain model: [04 — Domain model §4.13](04-domain-model.md#413-ai-run-decision-envelope-and-outcome)
