//! `smash-core` — framework-free domain and application services.
//!
//! Business logic (Memory lifecycle, Rules, retrieval router, authorization
//! decisions) lives here so worker tasks, MCP tools, and tests reuse the same
//! contracts. **This crate must never depend on Axum, Tower, SQLx, Reqwest,
//! or LanceDB.** That invariant is enforced by `cargo deny` (`deny.toml`
//! `bans` section), not by code review — see
//! `V2/docs/adr/0004-crate-boundaries-and-dependency-direction.md`.
#![forbid(unsafe_code)]

use smash_contracts::CONTRACTS_CRATE_PLACEHOLDER;

/// Phase A placeholder. Real application services land in later phases.
pub fn core_crate_placeholder() -> &'static str {
    CONTRACTS_CRATE_PLACEHOLDER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_against_contracts() {
        assert_eq!(core_crate_placeholder(), "smash-contracts");
    }
}
