//! `smash-storage` — SQLx, S3 (MinIO), and LanceDB adapters implementing
//! `smash-core`'s ports. Must never depend on Axum (see ADR-0004).

use smash_core::core_crate_placeholder;

/// Phase A placeholder. Real adapters (Postgres repositories, object-storage
/// client, LanceDB projection) land once the canonical schema exists
/// (Phase B) — this crate must not invent one now.
pub fn storage_crate_placeholder() -> &'static str {
    core_crate_placeholder()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_against_core() {
        assert_eq!(storage_crate_placeholder(), "smash-contracts");
    }
}
