# 20 — Post-Community Managed Service Focus

> Source: SMASH_V2.md §21

After Community Edition is credible and used, the managed service focuses on **operational and organizational capabilities rather than redefining Memory**.

## 20.1 Multi-tenancy and scale

Introduce production tenant provisioning over shared PostgreSQL, MinIO, and LanceDB services:

- shared PostgreSQL schemas with mandatory tenant IDs and row-level defense in depth;
- MinIO tenant prefixes or dedicated buckets;
- LanceDB namespace/table placement per tenant;
- per-tenant encryption and quotas;
- worker autoscaling;
- dedicated queues;
- object-storage lifecycle;
- rate limiting;
- regional placement and disaster recovery.

The **tenant placement registry** allows standard tenants to share infrastructure and selected enterprise or regulated tenants to move to dedicated PostgreSQL, object-storage, or vector placement **without changing domain IDs or APIs**.

Avoid one PostgreSQL partition or schema per small tenant by default. Partition high-volume event and trace tables according to **measured** query and retention behavior.

Topology: [23 — Diagrams §24.13](23-diagrams.md#2413-managed-tenant-topology).

## 20.2 Identity and SSO

Add:

- OIDC and SAML-based SSO;
- verified domains;
- Enterprise Admin and AI Governance Admin roles;
- invitations;
- group-to-Area mapping;
- session controls;
- SCIM or directory synchronization;
- service accounts;
- agent identities.

**Authorization semantics extend the Community schema rather than replace it.**

## 20.3 Enterprise governance

Add:

- retention policies;
- legal hold;
- audit export;
- SIEM integration;
- policy packs;
- approval chains;
- data residency;
- encryption-key management;
- connector administration;
- access reviews;
- compliance evidence.

## 20.4 Managed connectors and operations

Operate webhooks, background sync, credential rotation, connector health, replay, backfill, and support.

Offer managed extraction and embedding workers, model configuration, usage controls, and SLAs.

## 20.5 Collaboration and billing

Add team invitations, presence where useful, notifications, assignment, and review workflows.

**Price managed value around collaboration, governed activity, indexed Source volume, and expensive Aggressive-search work — not every trivial Light recall.**

## 20.6 AI decision intelligence

Build:

- AI Tracer;
- forensic replay;
- sandboxed execution reproduction;
- counterfactual replay;
- application-level decision analytics over the canonical decision ledger.

Provide enterprise dashboards that connect retrieved Memory and Rules to recommendations, application actions, business outcomes, and human corrections.

**This capability is tenant-owned.** Enterprise Admin and AI Governance Admin receive broad oversight according to policy; Area Admins and Normal Users remain scoped. ENGRAVE platform personnel have **no default content access**. Cross-tenant learning requires separate consent and privacy controls.

Detail: [16 — Observability and operations](16-observability-operations.md).
