# ENGRAVE Community Edition

The Community Edition is a single-node Docker Compose deployment for local or
small-team use. It is not an enterprise cluster, hosted MCP endpoint, OAuth
provider, or high-availability service.

## Install

Prerequisites are Docker Engine with Compose v2 and 8 GB of available memory.

```sh
cp .env.example .env
# Change every password and secret-like value in .env before sharing a host.
docker compose --env-file .env up --build -d
docker compose --env-file .env ps
```

The API, web application, PostgreSQL, and MinIO endpoints use the ports in
`.env`. The `migrate` service applies the ordered SQL migration chain before
the API and worker start. Re-running `up` is safe; do not use
`scripts/reset-local.sh` unless local data may be destroyed.

The Compose example sets `ENGRAVE_API_BIND=0.0.0.0` because the API listens
inside a container. For a host-run binary, leave the default loopback bind or
set `ENGRAVE_API_BIND=127.0.0.1` explicitly. Host port bindings default to
`127.0.0.1`; change the corresponding `*_HOST` variables only after adding an
authenticated reverse proxy and reviewed network policy.

## Configuration and boundaries

`.env` is local operator input and must not be committed. Keep PostgreSQL and
MinIO ports bound to localhost unless an authenticated reverse proxy and a
network policy are in place. Connector credentials are supplied to the worker
through the secret environment or an external secret store; they are not MCP
arguments, prompts, source content, or logs.

Light search is a bounded authorized retrieval operation. Aggressive search is
explicit, multi-step, and bounded by steps, elapsed time, tokens, candidates,
and external calls. It can return partial results, uncertainty, contradictions,
citations, and a durable trace. Cancellation is cooperative. Search never
activates Memory: proposals and review remain explicit governance actions.

Every result should be interpreted with its tenant, Area, Rule decision,
session/run identity, source version, coordinate, and content hash. Untrusted
source text is evidence, not policy, and connector output is quarantined before
disclosure. A Rule is enforced by the shared gateway and evaluator; prompts,
skills, and the web UI are not security boundaries.

The `workspace_setup` MCP capability provides one governed interview surface.
Use `begin`, `draft`, `confirm`, `submit`, `inspect`, or `cancel` with the
current session context. It returns structured JSON plus readable text,
rechecks authorization on every action, and `submit` creates only a pending
Map/Area Proposal. It never grants an Area or publishes a Map.

## Backup, restore, and upgrade

Back up PostgreSQL and MinIO together while the stack is running:

```sh
./scripts/backup-local.sh ./var/backups/$(date -u +%Y%m%dT%H%M%SZ)
```

The resulting directory is the versioned `engrave-community-recovery-v1`
paired recovery-export format. It contains PostgreSQL, MinIO, a `FORMAT`
marker, migration manifest, and checksums. It is a whole-deployment recovery
artifact, not a selective or redacted user-data export.

Restore replaces the current local database schema and object data. Verify the
checksum file first, then restore and start the stack:

```sh
./scripts/restore-local.sh ./var/backups/<timestamp>
docker compose --env-file .env up -d
```

For an upgrade, stop the old release, take a backup, replace the image/tag,
and run `docker compose up -d`. The migration service must complete
successfully before traffic is accepted. Keep the backup and previous image
until smoke tests and a restore rehearsal have passed. Roll back by stopping
the new release, restoring the paired backup, and starting the previous
release; migration rollback is restore-based, not an assumption that arbitrary
DDL can be reversed.

Maintainers can rehearse the complete disposable flow, including API health,
repeat migrations, backup, mutation, and restore-based rollback, with:

```sh
./scripts/rehearse-compose-release.sh
```

To run a locally built versioned image, set `ENGRAVE_IMAGE=engrave-v2:X.Y.Z`
and `ENGRAVE_WEB_IMAGE=engrave-v2-web:X.Y.Z` in `.env` before starting Compose.

## MCP and release artifacts

Local development can use `cargo run -p engrave-mcp`. A release supplies a
`engrave-community-X.Y.Z.tar.gz` operator bundle containing the Compose file,
migrations, plugin assets, documentation, and backup/restore scripts, plus one
standalone MCP binary per explicitly selected Rust target (native target by
default), SHA-256 checksums, and Registry metadata generated only from a
clean exact `vX.Y.Z` tag. For a release matrix, the owner sets
`RELEASE_TARGETS="aarch64-apple-darwin x86_64-unknown-linux-gnu ..."`; every
target must have a separately provisioned Rust toolchain/linker. The script
does not imply that cross-compilation or a target's support has been proven
until that target appears in the release evidence.
The artifact directory also contains a checksummed `BUILD_MANIFEST` tying the
release version and tag to the source commit, Rust compiler, selected targets,
and container image IDs.
The external approval and publication handoff is documented in
[`docs/release-review.md`](release-review.md).
`scripts/build-release.sh` never publishes. `scripts/verify-release.sh` checks
checksums, package metadata, common secret patterns, and starts the native
packaged MCP binary with `initialize`, then verifies clean EOF shutdown.
Metadata generation additionally requires `MCP_REGISTRY_TYPE`, which the
release owner must set only after confirming the current official schema. For
an official package type it also requires `MCP_REGISTRY_IDENTIFIER` naming the
separately published artifact; MCPB requires `MCP_REGISTRY_FILE_SHA256`.
`local-test` is the explicit synthetic-only mode used by disposable release
tests. For a separately published official package, set
`MCP_REGISTRY_VALIDATE_OFFICIAL=1` to call the Registry's non-publishing
validation endpoint through `scripts/validate-registry-official.py`. The script
never publishes or authenticates. Registry publication remains blocked until
the package type, security review, owner approval, and the official current
schema are verified.

### Connect an MCP host and two agents

Point the host at the unpacked native binary (or use `cargo run -p
engrave-mcp` during development). The bundled `plugin/.mcp.json` is a local
stdio example; replace its command with the absolute path to the released
`engrave-mcp-X.Y.Z-<target>` binary. Each tool call carries the governed
context below. Use a distinct `agent_identity_id` for each agent while keeping
the same tenant and Area when both agents should share that Area; the
PostgreSQL backend rechecks the actor, agent, session, and Area grant on every
call.

After the first Compose startup, an operator can create the local tenant,
Area, operator membership, and two agent identities with:

```sh
./scripts/bootstrap-local.sh local local-operator general agent-one agent-two
```

The command is idempotent for those slugs and prints three complete local MCP
contexts as JSON. Treat the output as local operator data; it is not an OAuth
token or proof of identity and must not be exposed outside the local host.

```json
{
  "context": {
    "tenant_id": "<tenant-uuid>",
    "actor_id": "<actor-uuid>",
    "host_id": "<host-name>",
    "agent_identity_id": "<agent-uuid>",
    "session_id": "<session-uuid>",
    "purpose": "review quarterly sales evidence",
    "role": "normal_user",
    "area_id": "<area-uuid>",
    "environment": "local"
  }
}
```

The smallest usable flow is: call `status`, queue an explicit
`ingest_source` with an `external_id`, call `recall` after the worker has
processed it, and call `propose_memory` with exact evidence references when a
durable observation is requested. A reviewer with an Area-authorized role
uses `review` with the proposal ID and expected version; approval is the only
step that can create active Memory. `recall` with `mode: "aggressive"`
additionally requires `explicit_intent: true` and a bounded task description.
A successful queue or proposal response is not a claim that content has been
approved or disclosed.

The [official Registry package-type documentation](https://modelcontextprotocol.io/registry/package-types)
is metadata-only and currently documents `npm`, `pypi`, `nuget`, `oci`, and
`mcpb` package types. A standalone native executable is not itself a Registry
package type, so ENGRAVE does not label its prebuilt binaries as a fabricated
package or publish them. A future release must first choose a supported
distribution (for example, a verified OCI image or MCPB artifact), publish it
separately, and then bind `server.json` to that exact artifact and checksum.

## Known limits

The deployment is single-node and local; backups are operator-managed; OAuth,
remote MCP, public Registry publication, multi-tenant hosted isolation, and
enterprise retention are not included. Identity/Area provisioning currently
requires an operator or an existing governed integration; there is no
self-service invitation or Area-grant UI. Invitation acceptance, Area-access
administration, pre-upload policy, and durable Source-only/Memory-Proposal
decision workflows are not yet part of this release; their underlying tables
and Rule hooks must not be mistaken for those user-facing flows. Retrieval
quality must be measured on a representative
corpus; the deterministic fixture is not a production quality claim.

Contributor and release-review instructions are in
[`CONTRIBUTING.md`](../CONTRIBUTING.md). A portable user-facing export format
and crates.io publication are not claimed by this Community Edition bundle;
the paired PostgreSQL/MinIO backup is the supported recovery artifact, while
external package distribution remains an owner-approved release gate.
