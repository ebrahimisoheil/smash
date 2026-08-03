# ADR-0005: Typed UUIDv7 identities

## Status

Accepted

## Context

The domain contains many identifiers that must not be confused at API or
storage boundaries: tenants, actors, Areas, Sources, Memories, Proposals,
Rules, Operations, AI Runs, and Events. The roadmap also requires identifiers
to remain stable when data moves from Community Edition to managed ENGRAVE.

Human-readable slugs are useful in URLs and the UI, but they are mutable,
non-unique across scopes, and not an authorization boundary. A single UUID
type would make it easy to pass an identifier for one aggregate where another
was expected.

## Decision

Every domain identifier is an opaque Rust newtype over UUIDv7. The initial
identifier family includes `TenantId`, `ActorId`, `MembershipId`, `RoleId`,
`AgentIdentityId`, `AreaId`, `MapId`, `MapVersionId`, `SourceId`,
`SourceVersionId`, `ArtifactId`, `ChunkId`, `EntityId`, `RelationshipId`,
`MemoryId`, `MemoryVersionId`, `EvidenceLinkId`, `ProposalId`, `RuleId`,
`RuleVersionId`, `EventId`, `OperationId`, `AiRunId`, and
`DecisionEnvelopeId`.

The newtypes:

- are opaque outside their defining module and cannot be mixed by accident;
- serialize and deserialize as canonical hyphenated UUID strings at API and
  fixture boundaries;
- implement equality, hashing, ordering, and database encoding through the
  wrapped UUID;
- are generated once at creation time and are never reused or reassigned;
- carry no authorization meaning by themselves.

UUIDv7 provides time-ordered values without exposing a business sequence.
Creation timestamps remain domain fields when the domain needs an authoritative
time; an ID's embedded ordering is only an indexing and locality aid.

Slugs and display names are separate presentation fields. Authorization and
data access use typed IDs plus explicit tenant/Area predicates, never slug
matching.

## Consequences

- Function signatures make cross-aggregate ID mix-ups compile-time errors.
- IDs remain portable between deployments and do not encode tenant, role, or
  other policy information.
- UUIDv7 improves insertion locality compared with random UUIDv4 while
  retaining opaque identifiers.
- Newtype boilerplate and explicit conversions are required in contracts,
  database adapters, and fixtures.
- A future identifier change requires a superseding ADR and a migration plan;
  callers must not infer semantics from the UUID representation.

## Alternatives rejected and why

- **Raw UUID fields everywhere** — rejected because the compiler cannot catch
  passing a `SourceId` to a function expecting a `MemoryId`.
- **Database-generated integer sequences** — rejected because they expose
  ordering, complicate offline fixtures and multi-writer deployments, and are
  less portable between CE and managed installations.
- **UUIDv4** — rejected as the default because UUIDv7 gives better index
  locality without making the identifier a security or business key.
- **Slugs as primary identifiers** — rejected because they are mutable,
  scope-dependent, and unsafe as an authorization boundary.

## Supersedes / superseded by

None.
