# ADR-0033: Phase G deterministic fixture and Rule harness

## Status

Accepted for Phase G.

## Decision

`engrave_core::fixture::generate` derives IDs, tenants, Areas, personas,
Sources, Memories, lifecycle/sensitivity labels, decoys, expected envelopes,
and benchmark metadata from one seed. It is credential-free and model-free;
the security oracle is deterministic. Default scale is 2 tenants, 3 Areas per
tenant, 5 personas, 50 Sources, and 500 Memories. Configuration supports
larger local and benchmark corpus sizes while preserving the seed.

The Rule harness starts at the core contract level with positive, negative,
locked-conflict, approval, and deterministic-envelope assertions. Retrieval
metrics remain recorded as Phase E baselines; Recall@20 is not a Phase G gate.

## Evidence and limitation

Same-seed and decoy tests pass in the workspace suite. Full historical-activity
dry-run UI and complete HTTP/MCP/connector killer-demo orchestration are not
yet present. These are explicit remaining Phase G work, not benchmark results.
