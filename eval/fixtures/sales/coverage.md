# Sales fixture coverage

| Lifecycle case | Expressed by | Assertion |
|---|---|---|
| Source versioning without mutating bytes | PDF versions 1 and 2 | Same logical Source has two immutable versions and one current pointer |
| Chunk identity = version + representation + coordinate + hash | Two call chunks | Both chunks have deterministic coordinates and content hashes |
| Evidence link integrity | Approved decision | Active approved Memory references both SourceVersions |
| Memory active state and applicability | Approved decision | Active Memory has validity bounds and approved origin |
| Contradiction in one applicability context | Contradicting Memory | Contradicting claim shares validity context and points to the approved Memory |
| Supersession and historical reconstruction | Superseding decision | New Memory points to predecessor with a human-readable reason |
| Proposal pending / approved / rejected-with-reason | Three proposals | All three review outcomes are present; rejection reason is retained |
| Rule effects `block` and `require_approval` | Sales Rule cases | Both policy effects are represented |
| Cross-Map proposal awaiting approval | Sales → Marketing mapping | Mapping remains `proposed` |
| Event emitted per mutation, in-transaction | Event rows | State changes carry idempotency keys and action/target pairs |
| Idempotent replay | Replay record | Repeated key returns the original result |
| Optimistic concurrency conflict | Concurrency record | Stale reviewed version produces `resource.version_conflict` |
| Trust provenance distinguishable | `origin` fields | `observed`, `inferred`, `proposed`, and `approved` are all present |
| Structured objects retain the Map version that interpreted them | Sales entities, relationship, and Cross-Map mapping | Every Entity/Relationship/Cross-Map row references a declared, published `map_version` |
