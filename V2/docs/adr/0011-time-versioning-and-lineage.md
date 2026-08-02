# ADR-0011: Time, versioning, and lineage conventions

Status: Accepted  
Date: 2026-08-02

## Decision

Persist instants as UTC `OffsetDateTime` values and expose RFC3339 timestamps.
Validity intervals are explicit (`valid_from`, optional `valid_until`) and are
not inferred from row creation time. Mutable aggregates use monotonic version
tokens; immutable versions point to their predecessor and record a reason for
supersession.

Historical reads select the version valid at the requested instant. A new Map,
SourceVersion, MemoryVersion, RuleVersion, or Decision Envelope never silently
changes the meaning of an older record.
