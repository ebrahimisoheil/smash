# 05 — Canonical Storage Responsibilities

> Source: the historical roadmap source §6

## 5.1 PostgreSQL is canonical

PostgreSQL is the source of truth for all structured and transactional state:

- tenants, users, enterprise roles, and memberships;
- Areas and Map versions;
- Cross-Map mappings;
- Source metadata and versions;
- extraction artifacts and chunk metadata;
- entities and relationships;
- Memory and Memory versions;
- evidence links;
- Proposals and review decisions;
- Rules and rule versions;
- connectors and credentials metadata;
- agent identities, sessions, AI runs, decision envelopes, and outcomes;
- Events, Operations, and idempotency records;
- lexical search projections where appropriate.

### Invariants belong in the database

Use database constraints to protect invariants rather than relying only on application conventions:

- unique logical identifiers;
- one current version pointer;
- valid lineage links;
- unique idempotency keys per scope;
- foreign keys that prevent evidence from pointing to a missing Source version.

### Tenancy

The managed default is **one PostgreSQL deployment with a shared schema** — not one database or schema per tenant.

- Every tenant-owned row includes an immutable `tenant_id`.
- Important indexes begin with or include `tenant_id`.
- Application queries always constrain tenant explicitly.
- PostgreSQL Row-Level Security provides defense in depth with **default-deny** policies.
- The application role must **not** own the protected tables and must **not** hold `BYPASSRLS` — table owners ordinarily bypass row policies.

Enterprise Admin, AI Governance Admin, Area Admin, Normal User, and agent permissions are resolved *within* the tenant. An Enterprise Admin's broader access changes the permitted Area and visibility set; it never permits access to a different tenant. ENGRAVE platform operators use separate operational identities and have no normal content-reading policy.

### Access and migrations

PostgreSQL is accessed through **SQLx**, with compile-time-checked queries against the canonical schema. An offline query cache is committed so builds do not require a live database.

Use **SQLx migrations from the first commit**. The database is never created from application model auto-generation in production. Every schema change needs a forward migration, compatibility considerations, and a tested backup/restore path.

Migrations run as a one-shot Compose service or an explicit command, and must complete before the API and worker accept work. See [06 §6.4](06-service-architecture.md#64-docker-compose-is-the-community-edition-product-unit).

### Lexical search

PostgreSQL full-text search provides the lexical candidate path for Light search. It is transactional, accessible without an additional service, and adequate for the Community Edition corpus. Search vectors are derived from clean text and metadata, and can be rebuilt.

### Job queue

For Community Edition background work, PostgreSQL also backs a durable job queue using claimed rows and safe locking semantics (`SELECT … FOR UPDATE SKIP LOCKED`). This avoids requiring Redis, RabbitMQ, or Kafka before the product needs them.

The queue is a **trait in the core crate** with a PostgreSQL implementation in the storage crate, so the managed service can adopt dedicated infrastructure without touching application logic.

## 5.2 MinIO owns binary objects

MinIO stores original Source bytes and large derived artifacts through an S3-compatible interface. Use **S3 semantics** rather than MinIO-specific behavior wherever possible so deployments can later use other compatible object stores.

Access goes through a standard S3 client crate behind an object-store port in the core crate. No MinIO-specific admin API is used on the request path.

### Layout

Buckets or prefixes distinguish: original Sources, derived artifacts, decision snapshots, exports, vector data, temporary uploads.

- Tenant prefixes are mandatory: `tenants/{tenant_id}/...`
- Object keys are generated from **stable IDs**, not user filenames. Human filenames remain metadata.
- Avoid flat, guessable paths.
- Never trust a client-supplied tenant or object key.

### Upload flow

Uploads use **staged objects**:

1. Client requests an upload intent.
2. Client receives a constrained upload mechanism.
3. Client completes the upload.
4. Client finalizes.

Finalization verifies size, media type, checksum, ownership, and expected object key **before** creating a Source version. Abandoned staging objects are cleaned through lifecycle policy.

### Credentials

MinIO root credentials are deployment bootstrap secrets and must not be used by the application at runtime. Create a **least-privilege service identity** limited to required buckets and actions.

In a multi-tenant deployment, isolation is enforced by application authorization and object-prefix policy, **not by obscurity**.

### Versioning and deletion

Object versioning and retention behavior must be documented. ENGRAVE application-level Source versions remain canonical even if the object store also versions objects.

Application hard deletion must coordinate PostgreSQL metadata, derived indexes, MinIO objects, and audit requirements.

## 5.3 LanceDB is a rebuildable retrieval sidecar

LanceDB stores vector and multimodal retrieval projections. **It is not the canonical Memory or Source database.** Every LanceDB row references a stable PostgreSQL ID and includes only the metadata necessary for safe prefiltering and retrieval.

LanceDB is written in Rust, so it enters the workspace as a **native crate dependency** behind a projection adapter in the storage crate — not as a foreign-language SDK or a network service. That convenience changes nothing about its status: it stays disposable and rebuildable.

### Tenant scoping

LanceDB is **tenant-scoped, not user-scoped**. A normal managed tenant receives one namespace or equivalent catalog boundary, such as `tenant_{tenant_id}`, under a tenant-specific object-storage path. Individual users receive permissions inside the tenant; they do not own separate canonical LanceDB folders.

Large or regulated tenants may later receive a dedicated LanceDB project or deployment **without changing canonical IDs**.

### Tables

Separate tenant tables for `memory_vectors` and `source_chunk_vectors`, because their ranking, lifecycle, and trust are different. Optional `entity_vectors` and `trace_vectors` are introduced only when their product value is proven.

Recommended fields: tenant, Area, canonical logical and version IDs, visibility class, status, validity, embedding model/version, content type, language, content hash, deletion marker.

### Security is prefiltering

Security filters are decided from **PostgreSQL authorization** and converted into LanceDB namespace selection plus metadata prefilters.

- Do **not** retrieve globally and remove unauthorized results afterward. Prefiltering is part of the security contract.
- The API must **rehydrate and re-authorize** every returned canonical record, because an index may be stale.

### Index state and reconciliation

Index state records belong in PostgreSQL: projection version, embedding model, dimensions, last successful rebuild, number of indexed records, error state.

A reconciliation job compares canonical records with LanceDB rows. **Full deletion and rebuilding must be routine operations.**

### Deployment

- Community Edition: single built-in tenant; persistent volume or an S3-compatible path.
- Managed: a tenant placement record resolves the namespace and object path. Application code must **never** construct an arbitrary path from untrusted request data.
- Only one well-defined indexing owner mutates a tenant table at a time — **the worker owns indexing writes; API processes query the index.**
- At higher scale, a managed or distributed LanceDB catalog can replace embedded access behind the same projection adapter.

## 5.4 Tenant provisioning and placement

PostgreSQL, MinIO, and the LanceDB catalog come up **once per deployment**. Creating an enterprise is an idempotent provisioning Operation inside those services, not a new hand-built infrastructure stack per customer.

### Provisioning steps

1. Create a `tenant` in `provisioning` state with opaque ID, region, and isolation tier.
2. Create the first Enterprise Admin membership and access policy.
3. Create Shared Memory, default Map, and default Rules.
4. Establish tenant MinIO prefixes for Sources, artifacts, decision snapshots, exports, vectors.
5. Create the LanceDB tenant namespace and its Memory and Source-chunk tables.
6. Record storage placement, schema, and embedding versions in PostgreSQL.
7. Verify database, object, and vector access using service identities.
8. Mark the tenant active, or retain an actionable failed state for safe retry.

See the state machine in [23 — Diagrams §24.14](23-diagrams.md#2414-tenant-provisioning-state-machine).

### Placement registry

The placement registry resolves `tenant_id` to: PostgreSQL cluster key, database, MinIO endpoint/prefix, LanceDB catalog/namespace, region, encryption-key reference, isolation tier.

Initially every managed tenant can point to the same shared services with different rows and prefixes. Later a large enterprise can move to dedicated PostgreSQL, MinIO, or LanceDB placement **without changing domain records or API semantics**.

### Isolation tiers

| Tier | Description |
|---|---|
| Community Edition | Single tenant |
| Standard SaaS | Shared infrastructure with logical isolation |
| Enterprise | Shared or dedicated placement |
| Regulated | Dedicated placement |

### Deletion

Tenant deletion is a coordinated, suspended-state Operation covering access shutdown, retention/export, Lance namespace removal, MinIO prefixes, PostgreSQL records, and proof of deletion.
