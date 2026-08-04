# Guided workspace and ontology interview

## Purpose

Give a person a conversational way to explain their work and turn that
explanation into a governed, reviewable Map/ontology draft without making the
agent the authority.

Example conversation:

1. “I work in fraud analytics.”
2. The agent asks about stakeholders, domains, and the work being organized.
3. The server returns Areas the current identity can use, such as `Fraud`,
   `Product`, or `Risk`, plus a request-new-Area option.
4. The agent presents a structured draft: kinds, relationships, Areas,
   assumptions, unresolved questions, and evidence.
5. The user selects or revises the draft.
6. Explicit confirmation submits a Map/Area Proposal. Nothing becomes active
   until the existing review/admission path approves it.

## Protocol shape

Use one broad MCP capability, `workspace_setup`, with an action discriminator:

```json
{
  "context": "<governed session context>",
  "action": "begin | draft | confirm | submit | inspect | cancel",
  "interview_id": "<required after begin>",
  "answers": {},
  "selected_area_ids": [],
  "requested_areas": [],
  "ontology_draft": {
    "kinds": [],
    "relationships": [],
    "assumptions": [],
    "unresolved_questions": []
  },
  "confirmed": false,
  "idempotency_key": "<replay key>"
}
```

The response should contain both:

- `structuredContent`: deterministic JSON for clients that can render forms,
  cards, choices, and confirmation controls;
- ordinary text/content: a readable Markdown summary for CLI and clients with
  no rich UI support.

XML is neither required nor trusted. A client may choose its own rendering.

## State machine

```text
started
  -> collecting
  -> draft_ready
  -> awaiting_confirmation
  -> submitted
  -> cancelled
```

`submitted` means a Proposal or Map draft was created. It does not mean an
Area was granted, a Map was published, or an ontology became active.

## Authorization boundaries

- `begin` lists only Areas visible to the current actor/agent/session.
- Selecting an inaccessible Area fails closed; the interview cannot use the
  conversation to manufacture access.
- `request_new_area` creates an Area request for an authorized reviewer; it
  does not create an active Area or grant membership.
- Every `draft`, `confirm`, and `submit` action reloads active Rules and checks
  the current Area envelope.
- Map publication, entity/relationship admission, and Area grants remain
  separate governed operations.

## Implementation order

1. Add core interview contracts and deterministic validation.
2. Add durable PostgreSQL interview state, replay keys, Area options, and
   Proposal links.
3. Expose one MCP `workspace_setup` capability with pre-hook and post-hook
   enforcement.
4. Add an `engrave-workspace` skill that asks useful questions but never
   claims access or approval.
5. Add rich structured output plus Markdown/JSON fallback tests.
6. Add live tests for Area narrowing, new-Area requests, confirmation replay,
   cancellation, stale Rules, and no silent activation.

## Current implementation audit

These lower-level pieces already exist:

- `memberships` and `area_grants` represent tenant membership and Area access;
- Source lifecycle states include quarantine and worker processing;
- Memory Proposals can be created explicitly and reviewed or rejected;
- the shared Rule gateway and disclosure checks provide enforcement points.

These user-facing workflows do not exist yet and must not be implied by those
tables or hooks:

- invitation creation, delivery, acceptance, expiry, revocation, or invite
  tokens;
- an Area access-request record, reviewer queue, approval, denial, or grant
  notification;
- an admin UI/API flow for managing memberships and Area grants;
- a pre-persistence content policy/DLP check that can block an upload because
  its content is forbidden. Current Rules can block a tool call, while Source
  quarantine is a later safety state; neither is the complete “do not upload
  or save this” product flow;
- a source-ingestion interview asking whether extracted observations should
  become Memory Proposals, remain Source-only, or wait for a batch decision;
- a durable user choice that distinguishes “create a Proposal,” “submit for
  review,” “approve now if authorized,” and “defer.”

The implementation must preserve this ordering:

```text
pre-upload policy decision
  -> upload or block
  -> Source quarantine/extraction
  -> candidate observations
  -> user decision: Source-only / Proposal / submit for review
  -> reviewer decision, if required
  -> active Memory only after governed admission
```

The pre-hook must run before an upload is persisted when the policy is a hard
content prohibition. The post-hook must quarantine/redact extracted output
before agent disclosure. Neither hook belongs in the skill text.
