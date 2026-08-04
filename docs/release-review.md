# ENGRAVE Community Edition release review

This checklist is the handoff boundary for a versioned Community Edition
release. A passing local command is evidence for the named property only; it
does not approve publication, hosted deployment, or production readiness.

## Local evidence

Run from a clean exact `vX.Y.Z` tag and record the output in
[`docs/phase-j-ledger.md`](phase-j-ledger.md):

- [ ] `cargo fmt --all`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] relevant ignored PostgreSQL, worker, MCP, connector, and Compose suites
- [ ] `npm --prefix apps/web audit --audit-level=high` and production build
- [ ] disposable install, repeat migrations, backup, restore, and rollback
- [ ] Light/Aggressive evaluation with its corpus-quality limitation recorded
- [ ] RustSec/advisory review and explicit dispositions
- [ ] `scripts/build-release.sh` and `scripts/verify-release.sh`
- [ ] `BUILD_MANIFEST`, `SHA256SUMS`, package contents, environment names,
      secret scan, and native MCP initialize/EOF smoke

The checked `BUILD_MANIFEST` is the source of truth for the release commit,
compiler, target list, and image IDs. The ledger must include the exact binary,
bundle, and image hashes for the owner-approved release commit.

## External approval gates

These cannot be inferred from local tests and must remain open until evidence
is attached:

- [ ] representative production corpus evaluation and retrieval-quality review
- [ ] external connector network isolation and credential-store/OAuth
      revocation review
- [ ] invitation, Area-access request, and membership/grant administration
      workflow review, or an explicit product deferral
- [ ] source pre-persistence policy and Source-only/Memory-Proposal decision
      workflow review, or an explicit product deferral
- [ ] supported package type selected and independently published
- [ ] package ownership, checksum/signature, repository identity, and Registry
      record verified against the exact release commit
- [ ] all intended target artifacts built and smoke-tested
- [ ] backup retention, restore-based rollback, and migration rollback review
- [ ] security reviewer signoff
- [ ] project-owner release approval

## Publication boundary

`scripts/build-release.sh` and the Community bundle never publish images,
packages, Registry metadata, or remote MCP. Do not set an official Registry
identifier until the selected package exists under the intended owner and its
checksum has been independently verified. Do not expose the single-node
Compose deployment to a public network while OAuth, audience validation,
tenant isolation, audit, rate limits, and revocation remain unreviewed.

## Approval record

```text
release_version:
release_commit:
artifact_manifest:
security_reviewer:
security_review_date:
owner_approver:
owner_approval_date:
publication_record:
rollback_record:
```
