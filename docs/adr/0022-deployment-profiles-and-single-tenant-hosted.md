# ADR-0022: Deployment profiles and single-tenant hosted

Status: Accepted
Date: 2026-08-03

## Context

SMASH should serve individuals, influencers, freelancers, teams, enterprises,
and regulated customers without creating separate domain models or forks. The
deployment may be self-hosted, operated for one customer, or shared across
customers. Data ownership, Area permissions, Maps, Rules, Memory, Events, and
stable identifiers must mean the same thing in each environment.

The current architecture already gives every tenant-owned record an immutable
`tenant_id`, defines tenant-scoped RLS defense in depth, and includes a
placement registry for database, object, vector, region, and key references.
Community Edition is a single built-in tenant. The managed-service design
currently describes shared infrastructure and later dedicated placement, but
does not explicitly name single-tenant hosted as a product profile.

## Decision

SMASH supports three deployment profiles over the same application and domain
contract:

1. **Single-tenant self-hosted** — one customer runs the complete deployment,
   normally through Docker Compose, with its own operational ownership.
2. **Single-tenant hosted** — SMASH operates a dedicated customer deployment
   or an infrastructure placement reserved for one tenant, including its
   database, object storage, vector placement, encryption boundary, backups,
   and operational controls as contracted.
3. **Multi-tenant hosted** — multiple tenants share services while remaining
   isolated by authentication, authorization, explicit tenant predicates, RLS,
   object prefixes, vector namespaces, quotas, and policy.

Deployment profile is placement and operations metadata, not a new tenant or
Memory model. A single-person account is still a tenant, usually with a
personal Area and optional shared Areas. A freelancer can keep personal and
client Areas in one tenant, or use separate tenants where ownership,
contractual isolation, billing, or export requires it.

The placement registry resolves the tenant to the required database, object
store, vector catalog/namespace, region, encryption-key reference, and
isolation tier. Moving a tenant between shared and dedicated placement must
not change domain IDs or API semantics. Dedicated placement must not bypass
the normal tenant, Area, Rule, or audit checks.

## Consequences

- Open-source Community Edition remains genuinely useful for self-hosting.
- Single-tenant hosted can be sold as a managed, dedicated, privacy- or
  residency-sensitive tier without a product fork.
- Multi-tenant hosting can optimize cost for smaller customers while retaining
  the same security contract.
- Provisioning, upgrades, backups, monitoring, billing, SSO, quotas, and
  support become deployment-profile concerns.
- Single-tenant hosted has higher operating cost and needs explicit placement,
  upgrade, restore, and break-glass procedures.
- The platform must never infer customer-content access from infrastructure
  placement; support access remains an explicit, audited grant.

## Required implementation gates

- Resolve authenticated actor, tenant, and agent identity before handlers.
- Provision and suspend tenants through idempotent Operations.
- Enforce tenant predicates and RLS in every hosted profile.
- Namespace MinIO and vector projections by tenant or dedicated placement.
- Test export/import and movement between self-hosted, single-tenant hosted,
  and multi-tenant hosted without changing stable identifiers.
- Expose the active deployment/isolation profile to administrators without
  exposing infrastructure secrets or private customer content.

## Current implementation status

The repository currently supports the direction in its contracts and schema,
not as a complete hosted product:

- Present: tenant-owned tables, tenant-scoped IDs and predicates in the
  repository scaffold, RLS policy definitions, named PostgreSQL/MinIO volumes,
  tenant-prefixed object keys, a placement table, and documented Community
  Edition versus managed tiers.
- Missing: authenticated tenant resolution, real API mutation handlers,
  tenant provisioning Operations, placement lookup, deployment-profile
  configuration, a non-owner application database role, and the hosted
  control-plane/operations needed for shared or dedicated deployments.
- Important local limitation: Compose currently connects through the
  PostgreSQL configured user, while the migration's RLS policy expects the
  application role not to own protected tables. RLS isolation is therefore not
  yet proven by the running API/worker stack and must be closed before hosted
  multi-tenancy or a security claim.

## Open questions

- Does single-tenant hosted receive a dedicated database by default, or may it
  use a logically isolated tenant in shared services at a lower tier?
- Which customers can request dedicated region, encryption keys, vector
  placement, or customer-managed infrastructure?
- What is the supported migration path from Community Edition to hosted and
  from shared hosted to dedicated hosted?
- Which billing, quota, and support guarantees differ by deployment profile?
- How are platform operator break-glass procedures reviewed and exposed to the
  customer in single-tenant hosted?
