//! `smash-contracts` — shared types, IDs, event and error models.
//!
//! This crate is the single origin for the published OpenAPI description and
//! for every other surface (`core`, `api`, `worker`, `mcp`) that needs to
//! agree on wire types. It must stay dependency-light: no web framework, no
//! SQL driver, no async runtime binding. See
//! `V2/docs/adr/0004-crate-boundaries-and-dependency-direction.md`.
#![forbid(unsafe_code)]

/// Phase A placeholder. Real contract types (Memory, Source, Rule, Operation,
/// error envelope, etc.) land in Phase B once the domain model is settled —
/// see `V2/docs/roadmap/04-domain-model.md`.
pub const CONTRACTS_CRATE_PLACEHOLDER: &str = "smash-contracts";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_is_stable() {
        assert_eq!(CONTRACTS_CRATE_PLACEHOLDER, "smash-contracts");
    }
}
