# Smash Roadmap

This document is the implementation plan for moving Smash from its current
Homebrew-first, Python-server-rendered, multi-surface state toward a
local-HTTP-API-first runtime with a stronger web product and a cleaner UI
architecture.

The goal is not a single rewrite. The goal is a sequence of controlled changes
that preserve the working core while separating business logic from transport,
then replacing packaging and UI layers one at a time.

Current date context: Saturday, August 1, 2026.

## Executive Summary

Smash already has one strong asset:

- the shared Python core in `mcp_package/smash_core/`

Smash currently has several weak or unstable outer layers:

- packaging and install path are biased toward Homebrew/macOS
- viewer/UI behavior is tied to prompt-copy workflows instead of direct actions
- `serve.py` mixes routing, rendering, API behavior, and product behavior
- the macOS Swift app (`apps/SmashBar`) is a separate surface that does not
  help the broader cross-platform web direction
- there is no first-class local HTTP API runner boundary
- there is no Docker-first local developer/runtime path

The path to success is:

1. stabilize the runtime boundary
2. extract a clean local HTTP API runner from the current viewer
3. improve the current UI in place only where it reduces user friction
4. build a new Next.js UI on top of the API
5. make Docker the primary execution surface for the API and UI
6. decide whether Swift survives as a thin native shell or gets retired

This roadmap is organized to let those steps happen one by one.

## Product Direction

### Desired End State

Smash should eventually work like this:

- a first-class local HTTP API runner exposes the product locally
- the Python core remains the source of truth
- `docker compose up` starts the local runtime
- the workspace is mounted into the container as local plain files
- MCP can be served from the same core/runtime without requiring Homebrew
- the main UI is a web application, ideally Next.js
- the UI performs real actions directly instead of asking users to copy prompts
- the runtime remains local-first, file-based, and offline-safe
- the primary local integration surface for UI clients is HTTP, not
  server-rendered HTML internals

### Non-Goals

These items are explicitly out of scope for the first refactor stages:

- replacing the Markdown/wiki storage model
- introducing a hosted backend
- moving business logic from Python into JavaScript
- rewriting memory rules, ranking, or governance during packaging work
- replacing every surface at once
- building a polished design system before runtime and API boundaries are fixed

## Architectural Principles

These principles should govern every phase:

1. The Python core remains the product kernel.
   - `mcp_package/smash_core/` should keep owning ingest, memory rules,
     validation, search, graph logic, and operations.

2. Transport and UI are replaceable shells.
   - `serve.py`, the API runner, Docker, Next.js, MCP, and Swift should all be
     treated as surfaces around the same core.

3. The API runner owns transport only.
   - business logic stays in `smash_core`

4. Runtime behavior must be reproducible.
   - a fresh machine should be able to run Smash without Homebrew

5. UI should trigger actions, not generate chores.
   - "copy this prompt and paste it elsewhere" is not a satisfying primary UX

6. Every phase must leave the repo shippable.
   - no branch should require a full rewrite to become usable again

7. Preserve local-first guarantees.
   - no hosted services
   - no hidden cloud dependency
   - no database migration away from plain files

## Current State Assessment

### Strengths

- Shared core exists and is real.
  - `mcp_package/smash_core/` already centralizes most business logic.
- The CLI is functional enough to prove the product.
- Validation and health concepts are strong.
- The product model is opinionated and coherent.

### Weaknesses

- Packaging is fragmented.
  - Homebrew is too prominent in docs and operational assumptions.
  - Python environment handling is brittle on managed interpreters.

- UI is useful but not productized.
  - the browser surface exposes information
  - too many flows depend on copyable commands/prompts
  - action execution is inconsistent

- `serve.py` is overloaded.
  - it owns web serving, API-like behavior, HTML composition, and some surface
    glue logic

- Surface drift is real.
  - recent fixes showed `serve.py` can drift from renamed core APIs

- Swift is platform-specific and strategically ambiguous.
  - it may be useful as a thin macOS shell
  - it should not drive the main UX direction if the target is local API + web

## Delivery Strategy

The refactor should be done in six major phases.

### Phase Overview

1. Phase 0: Stabilize and baseline the current repo
2. Phase 1: Introduce a first-class local HTTP API runner
3. Phase 2: Improve the existing browser UX around direct actions
4. Phase 3: Build a new Next.js frontend in parallel
5. Phase 4: Introduce Docker as the primary runtime
6. Phase 5: Migrate primary UI traffic to Next.js
7. Phase 6: Decide the fate of Swift/SmashBar

Each phase below includes:

- objective
- scope
- deliverables
- implementation tasks
- risks
- dependencies
- acceptance criteria
- rollback strategy

## Phase 0: Stabilize and Baseline

### Objective

Create a known-good baseline so the later refactor does not stack on top of
ambiguous repo behavior.

### Scope

In scope:

- fix obvious runtime drift between surfaces
- document current workflows
- define smoke checks for CLI, viewer, MCP, and demo

Out of scope:

- Docker
- Next.js
- Swift changes
- product redesign

### Deliverables

- working local CLI baseline
- working local viewer baseline
- working MCP baseline
- explicit smoke test checklist
- explicit architecture boundary note in docs

### Implementation Tasks

1. Audit current runnable surfaces.
   - `smash.py`
   - `serve.py`
   - MCP runtime in `mcp_package/smash_mcp`
   - `apps/SmashBar`

2. Fix known drift bugs.
   - import name mismatches
   - cache-induced validation confusion
   - packaging assumptions that prevent local startup

3. Add or strengthen smoke scripts.
   - CLI smoke
   - viewer smoke
   - MCP smoke
   - health/validation smoke

4. Document baseline commands.
   - local CLI launch
   - local viewer launch
   - MCP verify steps

5. Add a "known runtime matrix" note.
   - macOS + Homebrew Python
   - plain Python venv
   - future Docker target

### Risks

- This phase can expand forever if treated like cleanup.

### Mitigation

- Stop after the repo reaches a stable baseline.
- Do not redesign anything yet.

### Acceptance Criteria

- `smash.py try` works
- viewer starts reliably
- `verify-mcp` reports ready in a supported local setup
- validation can be repaired deterministically

### Rollback Strategy

- none required beyond reverting specific bug fixes

## Phase 1: First-Class Local HTTP API Runner

### Objective

Make the local HTTP API runner a first-class product surface instead of a
side-effect of the current viewer.

### Status

Status as of Saturday, August 1, 2026: in progress, materially advanced.

Implemented:

- dedicated loopback API runner at `api.py`
- shared route inventory and discovery payload wiring
- JSON-only `APIHandler` alongside the existing viewer handler
- CLI entrypoint for the API runner via `smash api`
- workspace/demo runtime copying for `api.py`
- human API docs in `docs/api.html`
- machine-readable OpenAPI document in `docs/openapi.yaml`
- live contract audit notes in `docs/api-contract-audit.md`

Verified live:

- `python3 api.py --root /tmp/smash-api-smoke --port 3013`
- successful live smoke on:
  - `GET /api`
  - `GET /api/health`
  - `GET /api/status?validate=true`
  - `GET /api/query-Smash`
  - `GET /api/graph-summary`
  - `POST /api/raw-source`
  - `POST /api/rebuild-backlinks`
  - `POST /api/rebuild-index`

Verified earlier in the same date window:

- broader live runner audit covering 36 successful endpoint calls across
  discovery, status, graph, ingest, memory, proposal, and mutation paths
- focused test pass for API/server coverage:
  - `tests/test_serve.py`
  - `tests/test_web_http_core.py`
  - `tests/test_demo_core.py`
  - `tests/test_cli_help_groups.py`

Not finished yet:

- the OpenAPI file is still too loose for strict client generation because many
  responses are still described as generic objects
- route metadata exists in multiple places and should be reduced to one source
  of truth
- Phase 1 should not be called complete until top-level payload contracts are
  made explicit per endpoint

### Why This Comes First

This directly addresses the architectural ambiguity:

- `serve.py` is currently both UI and backend
- Next.js needs a stable local API contract
- direct-action UI work needs a cleaner backend boundary

Without this step, every frontend or packaging move keeps coupling to the wrong
surface.

### Scope

In scope:

- formalize local HTTP API endpoints
- separate JSON API handling from HTML rendering
- define a dedicated local API runner surface
- preserve local-only security constraints
- keep the Python core as the domain layer

Out of scope:

- broad visual redesign
- Docker as the main runtime change
- removing legacy HTML pages immediately
- Swift changes

### Deliverables

- local API route inventory
- dedicated API runner entrypoint or module
- documented JSON contracts
- tests for core API endpoints
- compatibility story for existing HTML pages

### Design Decisions

1. The API runner should be local-only.
   - bind to loopback
   - preserve existing host/header protections

2. The API runner should own transport only.
   - business logic stays in `smash_core`

3. The current HTML viewer should become a client of the same backend paths
   where practical.

4. The API runner should be stable enough for Next.js to consume directly.

### Proposed API Surface

- status and health
- validation
- ingest status and ingest mutations
- rebuild index/backlinks
- query and brief
- memory inbox/review/archive/restore
- captures and proposals
- graph data

### Implementation Tasks

1. Inventory every existing `serve.py` route.

2. Classify routes into:
   - read-only JSON API
   - mutation JSON API
   - HTML page render
   - internal-only helper

3. Move API logic into `mcp_package/smash_core/web_http.py` or a new
   `api_*.py` module family.

4. Normalize response structures.
   - success
   - validation error
   - action result
   - mutation response

5. Normalize mutation semantics.
   - clear status codes
   - clear error payloads
   - idempotent rebuild endpoints where possible

6. Introduce a dedicated API runner path.
   - either a new entrypoint or a clearly separated mode within `serve.py`

7. Add API contract tests.
   - route behavior
   - mutation behavior
   - health/status shapes

8. Write API documentation.
   - inputs
   - outputs
   - local-only security model

### Risks

- extracting API logic may destabilize viewer behavior
- there may be hidden coupling between HTML rendering and route logic
- some current flows may depend on server-rendered assumptions

### Mitigation

- move route logic without changing behavior first
- test old and new route behavior against the same fixtures
- keep legacy pages during the extraction

### Acceptance Criteria

- a documented local API exists
- the API can drive health, validation, rebuild, and memory actions
- the current viewer still works
- Next.js can be built against the API without importing Python internals

Current read on criteria:

- documented local API exists: yes
- API can drive health, validation, rebuild, and memory actions: yes, verified
  live against `api.py`
- current viewer still works: likely yes based on shared-handler test coverage,
  but this should keep being checked as extraction continues
- Next.js can be built against the API without importing Python internals: mostly
  yes at the transport boundary, but the payload contracts still need tighter
  schema definitions before this should be treated as stable frontend ground

### Rollback Strategy

- keep the existing HTML viewer path available while the API surface is carved out

## Phase 2: Improve the Existing UI In Place

### Objective

Remove the worst UX friction now, without waiting for Next.js.

### Why This Matters

The current UI has useful information, but it often tells users what to paste
instead of letting them act. That is a product flaw independent of framework.

### Scope

In scope:

- convert copy-prompt flows into direct button-triggered actions
- improve navigation and state visibility
- preserve current server-rendered viewer

Out of scope:

- full frontend rewrite
- design system replacement
- React/Next.js migration

### Deliverables

- action-first health page
- action-first ingest page
- action-first memory review page
- action-first onboarding page

### UX Priorities

1. Health page should repair problems directly.
   - migrate
   - rebuild index
   - rebuild backlinks
   - validate

2. Ingest page should support direct source workflows.
   - add source
   - inspect pending
   - validate

3. Memory pages should support direct governance actions.
   - review
   - archive
   - restore
   - explain

4. Onboarding should trigger local setup actions where safe.

### Implementation Tasks

1. Audit every page for "copy this prompt" or "copy this shell command".

2. Replace passive recommendations with action buttons where the local viewer
   can safely perform the operation.

3. Make mutation feedback explicit.
   - success banners
   - failure details
   - refreshed status state

4. Improve page-level information architecture.
   - what is broken
   - what can be done
   - what happened after action

5. Reduce hidden navigation.
   - bring highest-frequency tasks into obvious paths

### Risks

- this phase can turn into a premature frontend redesign

### Mitigation

- focus only on action execution and navigation clarity
- avoid visual overhaul

### Acceptance Criteria

- users can recover health issues without leaving the UI
- users can inspect and act on ingest/memory flows directly
- copy-only workflows are no longer the primary path

### Rollback Strategy

- server-rendered pages remain simple to revert if a flow breaks

## Phase 3: Build Next.js in Parallel

### Objective

Create a new modern web frontend as a separate app that consumes the local API.

### Strategic Position

This should be a parallel surface first, not a replacement on day one.

### Scope

In scope:

- create `apps/web` or `apps/dashboard`
- use Next.js App Router
- consume the local API
- support core operational workflows first

Out of scope:

- moving business logic to JavaScript
- deprecating the old UI immediately

### Deliverables

- Next.js app scaffold
- local API client layer
- initial feature set with parity on core workflows
- local dev run path

### Initial Feature Targets

Release the new frontend in this order:

1. health/status dashboard
2. memory inbox
3. ingest status
4. graph explorer
5. onboarding page
6. query/brief workspace view

### Recommended Technical Direction

- App location:
  - `apps/web`

- Framework:
  - Next.js

- Rendering:
  - primarily client-side for action-heavy flows
  - server rendering only where it clearly improves startup/readability

- API access:
  - direct local HTTP calls to the Smash backend

- State:
  - simple fetch/state model first
  - avoid premature client state frameworks

- Styling:
  - clean local app shell
  - action-oriented dashboard, not docs page aesthetics

### Implementation Tasks

1. Scaffold Next.js app.

2. Add typed API client.
   - status
   - health
   - validate
   - rebuild
   - ingest
   - memory review
   - prompts/query/brief

3. Add local development proxy or env config.

4. Build shell UI.
   - workspace summary
   - nav
   - action feedback

5. Implement core pages in priority order.

6. Add tests.
   - route/page smoke
   - API integration tests

### Risks

- frontend can drift from backend semantics
- building too much UI too early can freeze API design

### Mitigation

- keep API contract versioned and tested first
- target operational parity before visual ambition

### Acceptance Criteria

- Next.js app can run locally
- health and inbox flows are usable end to end
- the app performs direct actions
- no business logic duplication in frontend

### Rollback Strategy

- Next.js is additive until primary cutover

## Phase 4: Docker-First Runtime

### Objective

Make Docker the primary recommended way to run the local API runner and web UI.

### Why This Comes Here

Once the API boundary is real and the Next.js app exists, containerization
becomes much cleaner:

- the API has a first-class process model
- the frontend has a clear client/server boundary
- Docker no longer needs to preserve `serve.py` as the product shell

### Scope

In scope:

- containerize API runner, CLI, and web app
- mount local workspace as a volume
- define compose services and dev workflow
- support persistent cache and data directories in mounted storage

Out of scope:

- replacing the Python core
- hosted backend changes
- removing non-Docker paths immediately

### Deliverables

- `Dockerfile` or Dockerfiles
- `docker-compose.yml`
- `.dockerignore`
- optional `docker/` helper scripts
- containerized startup docs
- container healthcheck strategy

### Design Decisions

1. The API runner should be its own process in the container model.

2. The workspace should be a bind mount.
   - users must keep plain files locally

3. The runtime image should not depend on Homebrew.

4. The Next.js frontend may be:
   - a separate Node service in development
   - optionally bundled differently later

5. MCP should remain runnable from the same core/runtime image set.

### Proposed Container Model

- Base images:
  - Python image for API/core
  - Node image for Next.js frontend if separate

- Runtime layout:
  - app code copied into `/app`
  - workspace mounted at `/workspace`

- Suggested services:
  - `smash-api`
  - `smash-web`
  - optional `smash-mcp`

### Implementation Tasks

1. Add Dockerfiles.
   - API/core runtime
   - frontend runtime if separate

2. Add `docker-compose.yml`.
   - mount workspace
   - wire API and frontend
   - expose local ports

3. Add make or shell helpers.
   - `docker compose up`
   - `docker compose run`

4. Verify file ownership and permissions.

5. Add health checks.
   - API health endpoint
   - frontend availability

6. Update docs.
   - make Docker the first local install path
   - downgrade Homebrew to optional/macOS convenience

### Risks

- workspace path assumptions may be scattered across the repo
- file permissions may create host friction
- MCP desktop integration may still need host-specific glue

### Mitigation

- use clear mount conventions
- run container as current user where practical
- keep MCP containerization additive at first

### Acceptance Criteria

- a fresh machine can run Smash with Docker only
- API works via container
- Next.js UI works via container
- CLI commands work via container
- no Homebrew is required for baseline usage

### Rollback Strategy

- Docker files are additive; local Python flow remains available

## Phase 5: Shift Primary UI to Next.js

### Objective

Make the Next.js app the default UI while reducing `serve.py` to a backend/API
host or compatibility surface.

### Scope

In scope:

- switch docs/onboarding/default recommendations to Next.js
- decide how the Python server hosts or pairs with the frontend
- maintain compatibility during transition

Out of scope:

- removing legacy UI immediately

### Deliverables

- production/development launch path for Next.js UI
- compatibility routing plan
- deprecation notice for old HTML UI if appropriate

### Implementation Tasks

1. Decide hosting model.
   - separate frontend container + backend container
   - or backend API + static Next.js bundle

2. Make default "open UI" path point to Next.js.

3. Keep old viewer accessible under a legacy path during migration.

4. Update docs and onboarding.

5. Add parity checklist and cutover audit.

### Risks

- partial parity may leave old UI necessary

### Mitigation

- keep legacy viewer available until key workflows are proven

### Acceptance Criteria

- users land in Next.js by default
- core workflows no longer require legacy pages
- operational docs point to Docker + web app first

### Rollback Strategy

- retain legacy UI endpoint until parity is stable

## Phase 6: Swift / SmashBar Decision

### Objective

Decide whether `apps/SmashBar` is:

- retained as a thin macOS-native convenience shell, or
- retired in favor of the web UI

### Decision Framework

Keep Swift only if it provides unique value that the web UI cannot reasonably
match, such as:

- native menu bar presence
- native notifications
- global shortcut integration

Do not keep it as a full independent product surface if that means duplicating
workflow investment.

### Option A: Keep Swift as a Thin Shell

If kept, Swift should do only:

- notifications
- menu bar badge
- quick open to web UI
- optional lightweight inbox shortcuts

The heavy UI should live in Next.js.

### Option B: Retire Swift

If retired:

- freeze it
- remove it from default docs
- keep source until confidence in web UX is high

### Acceptance Criteria

- there is only one primary UI investment direction

## Cross-Cutting Workstreams

### Workstream A: Documentation

Tasks:

- make the local API runner the primary architecture narrative
- make Docker the primary setup path once Phase 4 lands
- move Homebrew to optional/macOS convenience
- rewrite onboarding docs around action-based UI
- document API contracts
- document surface ownership clearly

Success criteria:

- a new contributor understands which layer to edit
- a user can run the product without Homebrew

### Workstream B: Testing and CI

Tasks:

- add API contract tests
- add Docker-based smoke tests
- add Next.js smoke tests once app exists
- keep existing Python regression tests green

Success criteria:

- each phase has automated proof of baseline behavior

### Workstream C: Runtime Boundary Discipline

Tasks:

- reduce drift between root scripts and shared core
- make wrapper surfaces thinner
- avoid feature duplication between shells

Success criteria:

- breaking a core API rename does not silently break one surface

### Workstream D: Security and Locality

Tasks:

- preserve local-only host protections
- keep outbound-network restrictions
- validate Docker networking assumptions
- preserve file-based trust model

Success criteria:

- API runner and Docker setup do not weaken local-first guarantees

## Suggested Milestones

### Milestone 1: "Stable Local API"

Target:

- local API runner exists
- route contracts documented
- current viewer remains functional

Exit:

- frontend work has a stable backend boundary

### Milestone 2: "Viewer Can Act"

Target:

- health, ingest, and review pages trigger actions directly

Exit:

- copy-prompt workflow is no longer primary

### Milestone 3: "Parallel Next.js App"

Target:

- Next.js health + inbox + ingest flows working

Exit:

- serious user evaluation of the new UI is possible

### Milestone 4: "No Homebrew Required"

Target:

- Docker runtime exists
- docs updated
- API and UI work in container

Exit:

- a user can ignore Homebrew completely

### Milestone 5: "Primary Web Cutover"

Target:

- Next.js becomes default UI

Exit:

- legacy UI is optional

## Recommended First Four Pull Requests

### PR 1: Local API Runner Boundary

Contents:

- route inventory
- extract API handlers from `serve.py`
- document local API contracts
- prove health/validate/rebuild flows through HTTP

Do not include:

- UI redesign
- Docker
- Swift changes

### PR 2: Action-First Health Recovery

Contents:

- improve current UI health page
- direct buttons for migrate/rebuild/validate
- explicit mutation feedback

Do not include:

- Next.js
- broad visual redesign

### PR 3: Next.js Scaffold Against Local API

Contents:

- scaffold `apps/web`
- add typed API client
- implement health/status page against local API

Do not include:

- Docker production setup
- broad parity work

### PR 4: Docker Runtime Skeleton

Contents:

- add Dockerfiles
- add compose file
- run API and Next.js in container
- add runtime docs

Do not include:

- backend logic changes
- product redesign

## Open Decisions

1. Should the API runner remain inside `serve.py` in a dedicated mode, or
   become a new entrypoint/module?
2. Should MCP be served from the same Docker container or a separate service?
3. Should the Next.js app be served by its own Node container, or exported and
   served behind the Python runtime?
4. Is Swift a native companion, or technical debt?
5. Should the repo remain single-package Python plus apps, or evolve into a
   clearer monorepo layout with `apps/` and `packages/` conventions?
6. Should the old server-rendered UI remain as a maintenance/debug surface?

## Success Metrics

### Developer Experience

- time to first run on a clean machine
- number of steps requiring host-specific package managers
- number of environment-specific setup failures

### Product Experience

- number of flows that require copying prompts or commands
- number of repair flows executable entirely from the UI
- number of core workflows available in the new web app

### Architecture Health

- count of business-logic duplications across surfaces
- count of surface drift failures
- test coverage on local API routes

## Final Recommendation

Do not start with Docker and do not start with Next.js alone.

Start here:

1. local HTTP API runner boundary
2. direct-action improvements in the current UI
3. Next.js parallel app
4. Docker runtime

That sequence improves current UX without a rewrite, creates the correct
backend boundary before frontend replacement work begins, and then removes
Homebrew lock-in at the runtime layer.

## Immediate Next Step

The next implementation step should be:

- extract and formalize the local HTTP API runner from the current viewer

That is the highest-leverage first move and the cleanest first PR because every
other desired change depends on it.
