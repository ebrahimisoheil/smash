//! `smash-mcp` — the MCP adapter over `smash-core`, sharing the API's
//! authorization path.
//!
use smash_contracts::TenantId;
use smash_core::{ApplicationError, Authorization};

struct NoopAuthorization;

impl Authorization for NoopAuthorization {
    fn authorize(
        &self,
        _tenant_id: TenantId,
        _action: &str,
        _target_type: &str,
    ) -> Result<(), ApplicationError> {
        Ok(())
    }
}

fn authorize_tool_call(
    auth: &impl Authorization,
    tenant_id: TenantId,
    action: &str,
    target_type: &str,
) -> Result<(), ApplicationError> {
    auth.authorize(tenant_id, action, target_type)
}

fn main() {
    authorize_tool_call(&NoopAuthorization, TenantId::new_v7(), "read", "memory")
        .expect("shared authorization path");
    println!("smash-mcp: authorization path ready");
}
