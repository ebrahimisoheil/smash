# 15 — Authentication, Authorization, and Security

> Source: SMASH_V2.md §16

## Start with the full contract

Community Edition begins with one tenant, one Enterprise Admin, and a documented local-only mode — but its **domain model uses the complete enterprise tenant and role contract**.

Introduce tenant, actor, membership, role, Area grant, visibility, and ownership fields **from the beginning**.

**Before public network exposure, require authentication.**

Managed ENGRAVE adds OIDC/SSO, enterprise membership, invitations, groups, SCIM or directory synchronization, service accounts, agent identities, policy and session administration. These are later **operational** capabilities — not reasons to postpone correct authorization boundaries.

## Implementation contract

| Concern | Choice |
|---|---|
| Token verification | **jsonwebtoken** — signature, expiry, issuer, and audience binding |
| Managed SSO | OIDC libraries layered on the same identity model |
| Enforcement point | A **Tower layer** resolves actor, tenant, and agent identity before any handler runs |
| Decision point | The **core crate** — authorization decisions are never made in an Axum handler |

Authentication middleware establishes *who is calling*. **It does not grant access to data.** Every application service still resolves the full authorization formula below against PostgreSQL.

Rust's guarantees cover memory safety, not policy. **A missing tenant predicate compiles perfectly.** Tenant scoping is enforced by explicit query constraints plus PostgreSQL Row-Level Security as defense in depth — see [05 §5.1](05-storage-responsibilities.md#tenancy).

Where `unsafe` appears in the workspace at all, it must be justified in code and reviewed; parser and extraction paths handling untrusted Source bytes prefer safe, well-maintained crates over hand-rolled decoding.

## The authorization formula

Authorization is the intersection of:

**tenant membership × enterprise role × Area membership × object visibility × purpose × Rules**

- Enterprise Admins can receive tenant-wide content and trace access.
- AI Governance Admins can receive tenant-wide decision oversight.
- Area Admins and Normal Users remain bounded to assigned Areas and object grants.
- Exceptional private Areas may exclude broad administrative roles when the enterprise configures that policy.

## Where authorization is checked

Authorization is checked **before**:

- query candidate generation;
- Source reads;
- Cross-Map traversal;
- mutation.

**Never retrieve unauthorized vector candidates and filter them only at the end.**

PostgreSQL is the authority for permissions. LanceDB carries prefilter metadata as a **projection**.

## Customer administration vs platform operation

A customer Enterprise Admin may inspect its own tenant according to its policy.

**A ENGRAVE platform operator does not gain customer-content access from infrastructure privileges.**

Support access uses explicit **break-glass grants** with tenant, purpose, scope, approval, short expiry, and immutable access events.

## Threats to protect against

- prompt injection inside Sources and MCP resources;
- malicious tool descriptions or connector payloads;
- cross-environment and Cross-Area leakage;
- insecure direct object references;
- token theft and token passthrough;
- memory poisoning and silent admission;
- file parser vulnerabilities;
- decompression and resource exhaustion attacks;
- unsafe export or public sharing;
- audit tampering;
- accidental destructive migrations.

## Secrets

- Secrets live outside source control and outside database plaintext where avoidable.
- Connector credentials are encrypted with a **rotatable application key** and isolated by environment.
- **Logs never contain raw tokens or private Source bodies.**

## Release gates

Every release runs:

- dependency, container, and static security scans — including `cargo audit` against the RustSec advisory database and a license/source policy check (`cargo deny`);
- `cargo clippy` with warnings denied, and a check that no unreviewed `unsafe` block has entered the workspace;
- authorization tests;
- malicious Source fixtures;
- MCP tool-injection tests;
- backup restoration checks.

**Security failures that could leak data block release.**

## Related

- Role model: [04 — Domain model §4.2](04-domain-model.md#42-enterprise-roles-and-memberships)
- Access diagram: [23 — Diagrams §24.15](23-diagrams.md#2415-enterprise-role-and-access-model)
- Security tests: [17 — Testing §17.4](17-testing-evaluation.md#174-security-tests)
