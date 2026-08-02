# 04 — Core Domain Model

> Source: SMASH_V2.md §5

## 4.1 Enterprise tenant and Memory environment

An **Enterprise tenant** is the customer organization and primary ownership boundary. It owns its users, roles, Areas, Sources, Memory, Rules, connectors, decision traces, retention settings, encryption configuration, and append-only event history.

- A tenant is identified by an opaque, immutable `tenant_id`.
- A human-readable slug is **presentation metadata** and must never be the security boundary.

The Enterprise tenant is different from the SMASH platform. An Acme Enterprise Admin may be authorized to oversee all Acme data, while a SMASH platform operator has **no automatic right** to read Acme content. Unified infrastructure means shared operational services, not shared human visibility.

Community Edition operates as one built-in tenant with one initial Enterprise Admin. The same tenant, role, and placement contracts exist from the beginning so Community Edition data can move to managed SMASH **without changing identifiers or meaning**.

## 4.2 Enterprise roles and memberships

Enterprise access is role- and Area-based. Minimum role model:

| Role | Authority |
|---|---|
| **Enterprise Admin** | Manages the enterprise and, according to enterprise policy, can inspect all Areas, Sources, Memory, Rules, agent traces, and decision analytics inside that tenant |
| **AI Governance Admin** | Inspects AI runs, decisions, evidence, Rules, approvals, replay, and analytics without necessarily managing billing or ordinary membership |
| **Area Admin** | Administers content, Map versions, Rules, review, and traces inside assigned Areas |
| **Normal User** | Uses assigned Areas, adds Sources, searches permitted Memory, reviews assigned Proposals, inspects permitted activity |
| **Agent or service identity** | Receives explicit machine scopes for Areas, Source classes, retrieval, proposals, and tools |
| **SMASH platform operator** | Maintains infrastructure; has **no default customer-content permission** |

An enterprise may configure exceptional private Areas that remain restricted from ordinary enterprise-wide administrators — a customer policy decision. The architecture must support a customer-controlled `read_all_tenant_content` capability as well as explicit exclusions.

Membership and permission decisions must be **queryable historical records**. A trace must be able to show which role and grant permitted an action at the time it occurred, even if the user's membership later changes.

## 4.3 Area

An Area is a bounded cognitive and authorization context. It owns a Map, Sources, Memory, entities, relationships, Area Rules, retrieval defaults, and membership or visibility settings.

Examples: Sales, Marketing, Product, a client engagement, a research project, a private leadership context.

Areas prevent all knowledge from collapsing into one vocabulary. They also provide an intuitive default retrieval boundary — an agent working on Sales searches Sales and permitted Shared Memory before exploring Product or Marketing.

An Area includes:

- stable ID and slug;
- name, description, and icon/color presentation metadata;
- lifecycle status;
- visibility and membership policy;
- current Map version;
- default light-search and aggressive-search budgets;
- default Memory admission policy;
- retention policy references;
- creation and update actor information.

## 4.4 Map

A Map defines the kinds of objects and relationships meaningful inside an Area. It is more than a graph legend — it is a **versioned semantic contract**.

A **Map kind** defines: name, description, aliases, required and optional properties, identity keys, display rules, sensitivity defaults, permitted relations.

A **relation** defines: source kind, target kind, direction, cardinality, inverse label, lifecycle, and whether an agent may infer it or only propose it.

Every structured object and relation records the Map version under which it was interpreted. A Map update creates a new version; existing data does not silently change meaning. A migration or reinterpretation process may propose updates, but its results are auditable and reversible.

## 4.5 Cross-Map

Cross-Map connects concepts across Areas without forcing every team into a universal ontology. It is an **explicit mapping registry**, not an automatic flattening mechanism.

Supported mapping types:

| Type | Meaning |
|---|---|
| `equivalent_to` | Two concepts have the same intended meaning in the approved context |
| `same_identity` | Two records represent the same real-world identity while retaining Area-local views |
| `broader_than` / `narrower_than` | One concept contains the meaning of another |
| `related_to` | Useful association without equivalence |
| `derived_from` | One object or concept is produced from another |
| `blocked` | Explicit prohibition on cross-Area traversal or identity resolution |

Each mapping records source and target Map versions, rationale, confidence, provenance, reviewer, visibility, and validity period.

Cross-Map expansion is **permission filtered before traversal**. A result preserves original Area labels so the agent does not falsely report that two teams use identical terminology.

Cross-Map is conservative: incorrect identity merges and permission leakage are more damaging than a missed connection. Agents may propose mappings based on repeated evidence, but active mappings require review or a narrowly defined admission policy.

## 4.6 Source

A Source is original evidence, or a stable reference to evidence: uploaded files, local file references, remote URLs, recordings, images, messages, connector objects, MCP resources.

- The canonical Source record stores **metadata and object references**, not large binary bodies in PostgreSQL.
- Original bytes are stored in MinIO.
- A **Source version** records content hash, size, media type, observed time, external modification time, origin, connector, access policy, object key, extraction status, and processing lineage.

**Source immutability** means a new external version creates a new Source version. It does not mutate the bytes behind an existing version. This preserves reproducible evidence spans and enables audit. A logical Source may point to its latest version for convenience.

## 4.7 Source artifact and chunk

**Artifacts** are extraction outputs: normalized text, OCR output, transcript segments, page images, spreadsheet tables, slide notes, document structure, visual descriptions. Artifacts are derived and can be regenerated when the extraction pipeline changes.

**Chunks** are stable, addressable regions of a Source version. A chunk needs a deterministic coordinate:

- page and bounding region;
- audio time range;
- message ID;
- spreadsheet sheet and cell range;
- slide number;
- document heading and character offsets.

Chunk identity combines Source version + extraction representation + coordinate + content hash.

Storage split:

| Store | Holds |
|---|---|
| PostgreSQL | Canonical chunk metadata and text needed for filtering, traceability, full-text search |
| LanceDB | Retrieval-oriented chunk projections and vectors keyed by the canonical chunk ID |
| MinIO | Large derived artifacts such as page images or full transcripts |

## 4.8 Entity and relationship

**Entities** provide stable identities within an Area: people, accounts, products, campaigns, projects, topics, or custom Map kinds.

Entity properties must not become an unbounded JSON dumping ground. Frequently queried identity, lifecycle, and security fields belong in explicit relational columns. Map-defined flexible properties may use validated JSON with a schema version.

**Relationships** are typed, versioned, attributable, and scoped. A relationship may be:

- observed directly in a Source;
- inferred by a processor;
- proposed by an agent;
- approved by a reviewer.

Those origins must remain distinguishable. The graph must not imply equal trust for every edge.

## 4.9 Memory

A **Memory** is a durable, governed claim intended for future agent reuse. It contains at least:

- stable logical ID and immutable version ID;
- environment and Area scope, including Shared Memory where appropriate;
- Memory type, title, and clean claim text;
- reason for remembering;
- status and review state;
- confidence and importance signals;
- `applies_when` conditions;
- visibility and intended audiences;
- valid-from, valid-until, review-after, and expiry fields;
- creator, proposer, reviewer, and agent/session attribution;
- supersession lineage;
- evidence references to exact Source spans;
- entity and relationship references;
- content hash and optimistic-locking version;
- retrieval text projection **distinct from** the clean claim.

**Memory types** initially preserve the useful V1 distinctions: `preference`, `decision`, `project context`, `fact`, `note`, `procedure`. V2 may add `signal`, `claim`, `instruction`, or `policy reference` **only when their lifecycle differs enough to justify a separate type**.

The **clean claim** must remain separate from **retrieval context**. Synonyms, surrounding text, aliases, and query-oriented descriptions can help retrieval but must not be presented as part of the approved claim, and must not be used to determine semantic contradiction without explicit rules.

## 4.10 Memory proposal

A **Proposal** is the write boundary between observation and durable Memory. It stores:

- proposed content and reason;
- evidence;
- target Area;
- suggested applicability;
- duplicate candidates;
- contradiction candidates;
- policy evaluation;
- proposer identity.

Proposals may be: accepted; edited and accepted; merged with an existing Memory; rejected; deferred; or converted into a Source-only observation.

**Rejection reasons are valuable evaluation data.** They use both structured categories and optional human notes.

## 4.11 Rule

A **Rule** is an enforceable policy evaluated **outside the language model**. It can apply before retrieval, before a tool call, after a tool result, before a Memory write, or at session end.

A Rule contains: scope, priority, trigger, effect, rationale, source authority, owner, lifecycle, tests, version.

Initial effects: `allow`, `warn`, `require_approval`, `block`.

Rules must define conflict resolution. A narrower Rule may **strengthen** a broader rule but must not silently weaken a locked global restriction.

Rules are **not arbitrary code** in the first Community Edition. Prefer a constrained declarative condition model over user-supplied executable scripts — this keeps evaluation explainable and removes a major security surface.

## 4.12 Event and operation

Every state-changing action appends an **immutable Event within the same database transaction as the change**. Events record: actor, agent, session, action, target, previous version, resulting version, reason, policy result, request ID, idempotency key, timestamp.

Long-running work is represented by an **Operation** or **Job** record. Upload processing, OCR, embedding, reindexing, aggressive search, import, export, and bulk Map migration expose explicit states, progress, and failure details. Jobs must be resumable or safely retryable.

## 4.13 AI run, decision envelope, and outcome

An **AI run** is a tenant-owned product record representing one bounded agent task — not merely an infrastructure trace. It links the acting user or service identity, agent and host versions, active Area, task, retrieval events, model invocations, Rule evaluations, approvals, tool calls, resulting decision, and eventual application outcome.

The immutable **decision envelope** is the minimum information required to reconstruct what an agent knew and why it acted. It references exact Memory versions, Source chunks, Map and Cross-Map versions, Rule versions, Prompt and Skill versions, model configuration, available-tool definitions, approvals, and outputs.

Large or sensitive bodies live as encrypted MinIO snapshots referenced by hash and classification; PostgreSQL stores the canonical relationships and queryable dimensions.

An **outcome** links an AI decision or tool action to what occurred in the application or business workflow: accepted recommendation, changed CRM record, published campaign, blocked disclosure, won opportunity, human correction or reversal.

### Long-running agent processes

A long-running agent process is a durable product workflow, not an unbounded
chat transcript. It has an Operation/Job for execution state, an AI Run for
agent task and governance context, checkpoints for safe resume, Events for
important actions, and Sources/Artifacts for retained evidence. The user can
inspect progress, pause, resume, cancel, or revoke the process.

The process may save useful evidence and checkpoints during execution, but it
does not silently turn every observation into Memory. Reusable facts,
preferences, decisions, workflows, Map changes, and Rules follow the relevant
proposal, confirmation, and Rule/Harness policy for their Area. A personal
Area may permit explicit agent proposal → user confirmation → durable write;
shared and Cross-Map changes remain more strictly governed.

These links make future analytics about actual AI behavior possible instead of limiting observability to tokens and latency.

See also: [16 — Observability and operations](16-observability-operations.md).
