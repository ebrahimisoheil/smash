# Security policy

## Scope

ENGRAVE Community Edition is a single-node Docker Compose deployment. The
security boundary is the shared identity, tenant/Area authorization, Rule
evaluator, and gateway in the backend. Prompts, skills, MCP descriptions, and
the web UI are not authorization mechanisms.

The Community Edition does not provide hosted multi-tenant isolation, OAuth,
remote MCP, enterprise key management, high availability, or managed backup
retention. Do not expose its unauthenticated local ports to the public
Internet.

## Reporting a vulnerability

Do not open a public issue for an unpatched vulnerability. Contact the project
maintainer privately with the affected version or commit, deployment shape,
reproduction steps, impact, and any logs or artifacts needed to reproduce it.
Do not include real credentials, personal data, or tenant content in a report.

The maintainer should acknowledge receipt, reproduce the issue in a disposable
environment, assess tenant/data impact, and publish a coordinated fix or
mitigation. A release is blocked by an unresolved issue that can bypass Rule
evaluation, cross-tenant isolation, credential isolation, provenance
integrity, or artifact verification.

## Operator requirements

- Copy `.env.example` to `.env`, replace every local placeholder secret, and
  never commit `.env` or connector tokens.
- Keep PostgreSQL and MinIO bound to localhost unless an authenticated,
  audience-validated boundary is added and reviewed.
- Treat connector content as untrusted evidence; do not allow it to become
  policy or tool instructions.
- Back up PostgreSQL and MinIO together using the documented scripts, and
  verify checksums before restore.
- Retain the previous image and backup until an upgrade smoke test and a
  restore-based rollback rehearsal pass.

## Release security gate

Release artifacts must come from a clean exact tag, have verified checksums,
contain no credentials, declare their environment-variable names without
values, record source/compiler/target provenance and container image IDs, and
pass both the RustSec and npm audit checks or carry an explicit owner-approved
advisory disposition. Registry metadata is generated only
after the current official schema and package type are confirmed. OAuth,
revocation, public-network connector behavior, Registry publication, and owner
approval are separate gates and must not be inferred from local tests.
