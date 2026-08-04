# Phase J evidence ledger

Status: Community Edition release tooling and operator documentation are
implemented. Phase J is not complete and no production-readiness claim is
made.

## Delivered in this pass

- Added paired PostgreSQL/MinIO backup and restore scripts with checksums and a
  migration manifest: `scripts/backup-local.sh` and `scripts/restore-local.sh`.
- Versioned the paired backup as the explicit
  `engrave-community-recovery-v1` recovery-export format with a checked format
  marker; it is documented as whole-deployment recovery, not selective data
  export.
- Added clean exact-tag release build and verification scripts for the MCP
  binary: `scripts/build-release.sh` and `scripts/verify-release.sh`; the
  release verifier now starts the native packaged binary, validates MCP
  `initialize`, and proves clean EOF shutdown.
- Added a versioned Community Edition operator bundle containing Compose,
  ordered migrations, plugin assets, docs, and backup/restore scripts; the
  release verifier checks its path-safe contents and secret patterns.
- Added the governed `workspace_setup` MCP capability with durable
  `begin`/`draft`/`confirm`/`submit`/`inspect`/`cancel` state, tenant/session
  binding, replay protection, authorized-Area narrowing, deterministic
  Markdown/JSON output, and proposal-only submission.
- Added the `engrave-workspace` skill as a conversational client guide; it
  explicitly keeps authorization and publication in the server-side governed
  path.
- Added a disposable full-stack install/upgrade/restore-based rollback
  rehearsal: `scripts/rehearse-compose-release.sh`; the harness now waits for
  the migration container on both the initial install and repeat-upgrade run
  before asserting application state.
- Added release-bound Registry metadata generation bound to an exact tag:
  `scripts/generate-registry-metadata.py`; the build script rejects dirty
  trees before artifact creation, and structural metadata/package consistency
  validation is provided by `scripts/verify-registry-metadata.py`. Synthetic
  `local-test` metadata is isolated from official generation, which requires a
  separately published package identifier (and MCPB checksum when applicable).
- Added opt-in non-publishing official Registry validation through
  `scripts/validate-registry-official.py`; it calls only the Registry
  `/v0.1/validate` endpoint and never authenticates or publishes.
- Kept exact release-tag provenance outside `server.json` in the checked
  `RELEASE_TAG` artifact file, avoiding non-schema extension fields in official
  Registry metadata.
- Added non-maintainer install, configuration, search-boundary, backup/restore,
  upgrade, MCP, and known-limit documentation in `docs/community-edition.md`.
- Added `CONTRIBUTING.md` with local setup, review checks, security boundaries,
  and exact-tag release instructions; it is now part of the verified operator
  bundle.
- Added a concrete MCP host/two-agent setup flow with the required governed
  context fields and Source → recall → proposal → review sequence; it states
  the current identity-provisioning and authorization boundaries explicitly.
- Added idempotent local bootstrap for a tenant, operator membership, Area,
  and two Area-granted agent identities; it prints complete local MCP contexts
  without claiming OAuth or hosted identity proof: `scripts/bootstrap-local.sh`.
- Added the Community Edition security boundary, vulnerability-reporting, and
  release security policy in `SECURITY.md`.
- Strengthened `scripts/verify-release.sh` to require every operator-bundle
  file and directory, exact release-tag/version binding, and both versioned
  image digests; omission and positive fixture tests cover the checks.
- Added a checksummed `BUILD_MANIFEST` binding the release version/tag to the
  source commit, compiler, targets, and image identities; the verifier checks
  those fields and cross-checks image IDs against `IMAGE_DIGESTS` and the
  packaged native target against `NATIVE_TARGET`.
- Added `docs/release-review.md` as an explicit owner/security/publication and
  rollback handoff checklist; unchecked external gates remain visible there
  and in this ledger.
- Added version-selectable `ENGRAVE_IMAGE` and `ENGRAVE_WEB_IMAGE` Compose
  tags; source builds remain the default for local installation.
- Upgraded LanceDB from 0.26.2 to 0.29.0, aligned Arrow 58.4, removed the
  RustSec-advisory `lru 0.12.5`, and upgraded SQLx from 0.8.6 to 0.9.0 to
  remove the `rsa` vulnerability from the locked graph.
- Added a four-case deterministic Light-vs-Aggressive contradiction corpus;
  Light exposed 0/3 contradictory cases and Aggressive exposed 3/3, with
  results recorded in `eval/results/phase-j-light-vs-aggressive-2026-08-03.json`.

## Verification completed

```text
cargo fmt --all                                      PASS
cargo test --workspace --locked                        PASS
cargo clippy --workspace --all-targets --locked -- -D warnings PASS
docker compose --env-file .env.example config --quiet PASS
disposable Compose postgres+migrate from empty volume PASS
DATABASE_URL=... cargo test -p engrave-storage --tests -- --ignored --test-threads=1 PASS
DATABASE_URL=... cargo test -p engrave-mcp --test live_phase_h -- --ignored --test-threads=1 PASS
DATABASE_URL=... cargo test -p engrave-worker -- --ignored --nocapture --test-threads=1 PASS
git diff --check                                    PASS
cargo test -p engrave-storage notion::tests          PASS
corpus Light/Aggressive contradiction evaluation      PASS (0/3 vs 3/3)
live worker stale-Rule mutation and Traverse fixture  PASS
paired PostgreSQL/MinIO backup, checksum, mutation, restore rehearsal PASS
recovery-export format marker/checksum validation PASS (`engrave-community-recovery-v1`)
updated recovery-export format live install/upgrade/restore rehearsal PASS (project engrave-phase-j-export-v1)
fresh Compose migration chain followed by repeat migration run PASS
cargo deny check licenses bans sources                PASS
cargo audit (isolated advisory DB)                   PASS (3 dispositioned warnings; 0 vulnerabilities)
npm audit --audit-level=high (apps/web)               PASS (0 vulnerabilities)
Next.js production build (16.3.0)                    PASS
LanceDB 0.29 storage/MCP/worker regression suites    PASS
connector local credential revocation fail-closed test PASS (no network request after revoke)
disposable PostgreSQL Phase F/H/I/MCP/worker suites  PASS (project engrave-phase-j-live3)
clean full-stack Compose install/upgrade/rollback     PASS (project engrave-phase-j-clean14)
native MCP release binary initialize/EOF smoke         PASS (aarch64-apple-darwin, current tree)
Community Edition bundle path/contents/secret check    PASS (current tree)
workspace_setup durable/replay/proposal-only live test PASS (project engrave-phase-j-workspace2)
disposable local identity/Area bootstrap and complete MCP context output PASS (project engrave-phase-j-bootstrap)
disposable exact-tag release build and artifact verification PASS (v0.1.0; synthetic `local-test` registryType; bundle includes bootstrap)
clean Compose install/upgrade/backup/restore rehearsal with explicit upgrade-migration wait PASS (project engrave-phase-j-upgrade-wait)
release verifier positive fixture and required-bundle-entry omission test PASS
Community Edition bundle contributor-guide inclusion and secret scan PASS
Registry metadata positive/negative validation PASS (required transport, supported package types, tag/version binding)
official Registry non-publishing schema validation PASS (hypothetical OCI metadata; `POST /v0.1/validate`)
corrected exact-tag artifact completion and official validation PASS (v0.1.0; hypothetical OCI identifier; no publication)
fresh clean exact-tag release build and official validation PASS (v0.1.0; hypothetical OCI identifier; no publication)
current-worktree exact-tag release build and official validation PASS (v0.1.0; contributor/recovery/verifier/review updates included; hypothetical OCI identifier; no publication)
current-worktree exact-tag release build with checked BUILD_MANIFEST PASS (v0.1.0; review checklist included; official non-publishing validation; no publication)
release-review checklist bundle inclusion and secret scan PASS
native target/BUILD_MANIFEST consistency assertion PASS
fail-closed release rejection tests PASS (dirty exact tag; missing official package identifier)
```

The disposable live run used Compose project `engrave-phase-j-live3`,
PostgreSQL 16 Alpine, host port 55445, a fresh named volume, and was torn down
with `down -v` after the suites passed. The empty migration run used project
`engrave-phase-j-verify` and applied all 12 ordered migration files.
The backup/restore rehearsal used Compose project `engrave-phase-j-backup4`,
PostgreSQL host port 55434, MinIO ports 59002/59003, and sidecar image
`alpine:3.20`; it restored the pre-mutation tenant slug and verified both
checksum files. The repeated migration rehearsal used project
`engrave-phase-j-upgrade` and host port 55435.
The workspace interview live proof used disposable project
`engrave-phase-j-workspace2`, host port 55449, and verified idempotent begin,
draft/confirm/submit transitions, actor/agent/session binding, one pending
`map_area` Proposal, and no new Memory row.
The local bootstrap proof used disposable project
`engrave-phase-j-bootstrap`, host port 55452, applied the full migration chain,
created/upserted one tenant, operator membership, Area, and two agent grants,
replayed the bootstrap to verify stable identities/grants, and emitted three
complete JSON MCP contexts before teardown with `down -v`.
The full-stack rehearsal first exposed two harness/application defects: the API
was bound to loopback inside its container, and the rehearsal inspected only
running migration/API container IDs. The API now accepts an explicit
`ENGRAVE_API_BIND` (Compose uses `0.0.0.0`; host-run defaults remain loopback),
and the harness uses `ps -aq` for exited migration and restored services. The
corrected run used project `engrave-phase-j-clean14`, host ports 55451, 59026,
59027, and 33011, with localhost-only host bindings, and completed all
assertions after the SQLx 0.9, LanceDB 0.29, and Phase J workspace-interview
migration upgrades. Its manifest recorded:

```text
postgres_image=sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777
api_image=sha256:de1e0d9012d2421dc394c8728c9a0e3bba9181b912c370ca1fd0ccc2126d2d40
web_image=sha256:ad7dc8f825c93d55eab390c11ac34ff039ae6603992347a360bc8678bf87657a
```

The disposable exact-tag release rehearsal created a temporary clean commit
tagged `v0.1.0`, built the native `aarch64-apple-darwin` MCP artifact, built
both versioned Docker images, generated checksums and the operator bundle, and
passed `scripts/verify-release.sh`. It intentionally used
`MCP_REGISTRY_TYPE=local-test`; this proves the artifact pipeline and verifier,
not official Registry schema approval, package publication, or owner signoff.
The latest artifact evidence recorded:

```text
engrave-v2:0.1.0 sha256:9fef5e0aab7fc3a74e2030674264ca44d470b7da697192057690eaaa58669f26
engrave-v2-web:0.1.0 sha256:c33747e16628fcb9baf3bd4c9282ee91c373ae6f81c4ff37cc72ef679f05ff61
5dcd96bf22aa54424985299012331b97e027b3e78a8e8481775d3a47ce35b848  engrave-mcp-0.1.0-aarch64-apple-darwin
9a6caa88676a741cb02a1c6d0f6f0d0b8d90e4fd1bd94b58c30e0aaf5ebf0468  engrave-community-0.1.0.tar.gz
```

The hardened Compose rehearsal used project `engrave-phase-j-upgrade-wait`,
host ports 55461, 59036, 59037, and 33021, and tore down its disposable
volumes on exit. It waited for the migration container to exit successfully
both before the initial health assertion and after the repeat migration
startup. Its manifest recorded:

```text
postgres_image=sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777
api_image=sha256:e04a9f8a98218d93c6439b4ceac0def9f93ac6993c7bf6ac7c18b3176f029e00
web_image=sha256:c7c5090e63c50b71e3e031e27cb626d57b0250f9aa3137fdbc864f179dd07e5c
```

The updated recovery-export rehearsal used project
`engrave-phase-j-export-v1`, host ports 55462, 59038, 59039, and 33022. It
verified the `FORMAT` marker and checksum entries for the PostgreSQL dump,
MinIO archive, and marker during restore, then repeated migrations and proved
the restored tenant state and API health. Its image manifest recorded:

```text
postgres_image=sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777
api_image=sha256:60b177686efb1fd277f14ecad9a5087a7854888babaf259ca2561fd65bc6f97d
web_image=sha256:aaa1ca2dc704c5c361024096be204dc12fc9b77adae177856b9cd58bd4deb614
```

The current-worktree exact-tag release rehearsal used a clean temporary tree
tagged `v0.1.0` and included the contributor guide, recovery-export marker,
strengthened bundle verifier, and checked `BUILD_MANIFEST`. Workspace tests,
clippy, npm audit, Next production build, both versioned Docker images, local
metadata validation, official non-publishing validation, checksum generation,
bundle inspection, and native MCP smoke all passed. The manifest recorded:

```text
version=0.1.0
release_tag=v0.1.0
source_commit=8ed49412aeb548542dddd7382464dc0ab183aa4a
native_target=aarch64-apple-darwin
release_targets=aarch64-apple-darwin
rustc_version=rustc 1.97.1 (8bab26f4f 2026-07-14)
image_engrave-v2:0.1.0 sha256:7f571b70a04098dabcbc5db561bef1ebdd0514bee29407e8a659539b33413d89
image_engrave-v2-web:0.1.0 sha256:2887f56e7d31984cdbda70524a9dffef14a5a305d6e0a8c4310099deff1dd269
```

The corresponding artifact hashes were:

```text
engrave-v2:0.1.0 sha256:7f571b70a04098dabcbc5db561bef1ebdd0514bee29407e8a659539b33413d89
engrave-v2-web:0.1.0 sha256:2887f56e7d31984cdbda70524a9dffef14a5a305d6e0a8c4310099deff1dd269
7060ed38dfe9e40304da98d496dfd53d450c9bd34fcd1dfa3765db8011d1dd25  engrave-mcp-0.1.0-aarch64-apple-darwin
bedae8998beb8b56dc4f889c41cf8e9ea79b745fd5599b787fb73b23e0614c9e  engrave-community-0.1.0.tar.gz
```

After removing the schema-invalid internal tag extension from `server.json`, a
fresh disposable `v0.1.0` tree was built with the corrected `RELEASE_TAG` file
and current bundle/verifier inputs. The complete release script ran from the
clean exact tag, including workspace tests, clippy, npm audit, Next production
build, both Docker image builds, local metadata validation, official
non-publishing validation, checksum generation, bundle inspection, and native
MCP smoke. All passed:

```text
engrave-v2:0.1.0 sha256:ed96773bc4e97fe750a18401f289fbc8020505792f22361eab694b5c371dc85e
engrave-v2-web:0.1.0 sha256:b5af49c7469776a754ce4c5afcb35d4411fe383c1702b722330b74609f6d1a9f
5dcd96bf22aa54424985299012331b97e027b3e78a8e8481775d3a47ce35b848  engrave-mcp-0.1.0-aarch64-apple-darwin
ed725bbf3275761c6f50b79f846ebf18d1c3db7cae4669dd1fcaa93c9c524fd3  engrave-community-0.1.0.tar.gz
```

## Required evidence still pending

- A representative production corpus evaluation remains pending; the new
  four-case deterministic corpus closes the Phase I contract gap but is not a
  production retrieval-quality claim.
- External credential-store/OAuth revocation and public-network connector
  isolation evidence; local timeout, retry, tenant binding, explicit
  fail-closed revocation, prompt injection, quarantine, and secret-pattern
  checks now pass.
- Invitation acceptance, Area access-request review, membership/Area-grant
  administration, pre-persistence source policy, and durable Source-only vs
  Memory-Proposal decisions remain product workflow gaps rather than implied
  capabilities of the existing tables or Rule hooks.
- Release-time Registry schema/package-type recheck for the selected real
  package, package ownership verification and publication, security review,
  owner approval, rollback review, and release artifact checksums from the
  real owner-approved release commit. The current schema endpoint accepts the
  hypothetical OCI metadata, but no supported public package has been selected
  or published for the native MCP binary. The disposable exact-tag artifact
  pipeline and non-publishing validation pass, and the release script is
  target-selectable, but publication, approval, and any additional cross-target
  artifacts remain unverified.
- The roadmap’s crates.io distribution and portable user-facing export format
  are explicitly not claimed by this Community Edition pass: the workspace is
  still `publish = false`, and paired PostgreSQL/MinIO backup is the supported
  recovery artifact. Selecting and implementing a public package/export
  channel requires separate owner and security review.

These are explicit blockers, not inferred from the passing baseline suites.
Registry publication and remote MCP remain intentionally undispositioned until
their external security and owner-approval evidence exists.

## Security disposition

The isolated RustSec audit command (`CARGO_HOME=<temporary-directory> cargo
audit`) now exits successfully with zero vulnerabilities and three
unmaintained-crate warnings in the locked dependency graph:

- `rsa 0.9.10`, RUSTSEC-2023-0071 (Marvin timing side channel), and
  `lru 0.12.5`, RUSTSEC-2026-0002 (unsound `IterMut`), are no longer present
  after upgrading SQLx to 0.9.0 and LanceDB to 0.29.0; the locked graph now
  uses LanceDB's `lru 0.16.4`.
- `bincode 2.0.1`, RUSTSEC-2025-0141, and `encoding 0.2.33`,
  RUSTSEC-2021-0153, are unmaintained transitive LanceDB/Lindera dependencies;
  both are explicitly dispositioned in `deny.toml` and are not direct ENGRAVE
  serialization inputs.
- `paste 1.0.15`, RUSTSEC-2024-0436 (unmaintained), remains explicitly
  dispositioned in `deny.toml` as a transitive LanceDB/DataFusion warning.

`cargo deny check advisories licenses bans sources` and the isolated `cargo
audit` both pass with the three transitive warning dispositions. Release
security signoff remains open for the external connector/OAuth review, artifact
review, and owner approval gates.

Connector timeout, bounded retry, non-retryable authorization failure, tenant
binding, prompt-like source quarantine, and secret-pattern checks are now
covered locally. OAuth revocation semantics, external credential-store
isolation, and public-network quarantine remain release blockers because they
require an actual credential authority and external connector/security review.

## Requirement audit matrix

| Phase J requirement | Authoritative evidence | Status |
|---|---|---|
| Non-maintainer Compose install/configuration | `docs/community-edition.md`; disposable Compose install | PASS |
| Add Sources, review Memory, two-agent MCP flow | live MCP/worker suites; `scripts/bootstrap-local.sh`; operator flow docs | PASS locally; hosted identity remains out of scope |
| Light/Aggressive retrieval, provenance, budgets, cancellation, partial results | core/unit/live aggressive suites; eval result; operator docs | PASS as deterministic contract; production corpus OPEN |
| Rule enforcement and no silent Memory activation | core gateway tests; live Rule/stale-authorization suites | PASS locally |
| Backup, restore, recovery export, and migration upgrade | `engrave-community-recovery-v1`; `engrave-phase-j-export-v1` rehearsal | PASS locally |
| Versioned containers and native MCP binary | exact-tag release rehearsal; `BUILD_MANIFEST`, image IDs, SHA-256 hashes | PASS for `aarch64-apple-darwin` |
| Additional platform artifacts | `rustup target list --installed`; release target matrix | OPEN; only native target is installed/evidenced |
| Connector isolation, timeout, retry, quarantine, local revocation | storage connector tests and live suites | PASS locally; OAuth/credential authority review OPEN |
| Registry metadata generation and schema validation | exact-tag generator/verifier; official `/v0.1/validate` | PASS non-publishing; package publication/ownership OPEN |
| Package contents, env names, secrets, provenance | `scripts/verify-release.sh`; positive/negative fixtures; `BUILD_MANIFEST` | PASS locally |
| Security, rollback, and release review | `SECURITY.md`; `docs/release-review.md`; local rehearsals | Local evidence PASS; independent security/owner signoff OPEN |
| Phase I product workflow gaps | ledger disposition and known limits | Explicitly deferred: invitations, Area requests/admin, pre-persistence policy, Source-only decision flow |

This matrix is the completion audit, not a production-readiness assertion. Any
OPEN row or unchecked external item in `docs/release-review.md` prevents Phase
J from being marked complete.

## Commands for the final gate

```text
cargo fmt --all
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
npm --prefix apps/web audit --audit-level=high
docker compose --env-file .env up --build -d
./scripts/backup-local.sh ./var/backups/<timestamp>
./scripts/restore-local.sh ./var/backups/<timestamp>
RELEASE_TARGETS="$(rustc -vV | sed -n 's/^host: //p')" ./scripts/build-release.sh <version> dist/<version>
./scripts/verify-release.sh dist/<version>
git diff --check
```

These commands are a checklist, not evidence until their output, artifacts,
checksums, and disposable infrastructure details are recorded here.
