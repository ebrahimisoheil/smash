//! Connector contracts shared by the worker and native adapters.
//! The interactive MCP server only queues work; this state machine is safe to
//! drive from a durable worker with tenant-scoped credentials.
#![forbid(unsafe_code)]

use engrave_contracts::TenantId;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalObject {
    pub external_id: String,
    pub title: String,
    pub content: String,
    pub permissions: Vec<String>,
    pub deleted: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectorCursor(pub Option<String>);
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceVersion {
    pub external_id: String,
    pub version: u64,
    pub checksum: String,
    pub permissions: Vec<String>,
    pub deleted: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncBatch {
    pub next_cursor: ConnectorCursor,
    pub versions: Vec<SourceVersion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectorCredentialRef {
    pub tenant_id: TenantId,
    pub connector: String,
    /// Opaque secret-store handle; the secret value never enters sync records.
    pub secret_handle: String,
}

pub trait CredentialStore: Send + Sync {
    fn resolve(&self, reference: &ConnectorCredentialRef) -> Result<(), String>;
}

pub trait ReadConnector: Send + Sync {
    fn name(&self) -> &'static str;
    fn list(
        &self,
        cursor: &ConnectorCursor,
    ) -> Result<(ConnectorCursor, Vec<ExternalObject>), String>;
}

/// Idempotent source-version projection. The key is tenant + connector +
/// external ID in the real repository; content changes append a version.
#[derive(Default)]
pub struct SyncLedger {
    versions: BTreeMap<(String, String), Vec<SourceVersion>>,
}
impl SyncLedger {
    pub fn apply(
        &mut self,
        tenant: &str,
        connector: &str,
        object: &ExternalObject,
    ) -> SourceVersion {
        let key = (format!("{tenant}:{connector}"), object.external_id.clone());
        let checksum = format!("sha256:{:x}", Sha256::digest(object.content.as_bytes()));
        let history = self.versions.entry(key).or_default();
        if let Some(current) = history.last() {
            if current.checksum == checksum
                && current.deleted == object.deleted
                && current.permissions == object.permissions
            {
                return current.clone();
            }
        }
        let version = history.last().map_or(1, |v| v.version + 1);
        let result = SourceVersion {
            external_id: object.external_id.clone(),
            version,
            checksum,
            permissions: object.permissions.clone(),
            deleted: object.deleted,
        };
        history.push(result.clone());
        result
    }
    pub fn sync<C: ReadConnector>(
        &mut self,
        tenant: &str,
        connector: &C,
        cursor: &ConnectorCursor,
    ) -> Result<SyncBatch, String> {
        let (next, objects) = connector.list(cursor)?;
        Ok(SyncBatch {
            next_cursor: next,
            versions: objects
                .iter()
                .map(|object| self.apply(tenant, connector.name(), object))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(Vec<ExternalObject>);
    impl ReadConnector for Fixture {
        fn name(&self) -> &'static str {
            "notion-source"
        }
        fn list(
            &self,
            _: &ConnectorCursor,
        ) -> Result<(ConnectorCursor, Vec<ExternalObject>), String> {
            Ok((ConnectorCursor(Some("2".into())), self.0.clone()))
        }
    }
    fn object(content: &str) -> ExternalObject {
        ExternalObject {
            external_id: "page-1".into(),
            title: "Roadmap".into(),
            content: content.into(),
            permissions: vec!["area:sales".into()],
            deleted: false,
        }
    }
    #[test]
    fn duplicate_is_idempotent_and_change_versions() {
        let mut l = SyncLedger::default();
        let c = Fixture(vec![object("v1")]);
        let a = l.sync("tenant-a", &c, &ConnectorCursor(None)).unwrap();
        let b = l
            .sync("tenant-a", &c, &ConnectorCursor(Some("2".into())))
            .unwrap();
        assert_eq!(a.versions[0], b.versions[0]);
        let c = Fixture(vec![object("v2")]);
        let d = l
            .sync("tenant-a", &c, &ConnectorCursor(Some("2".into())))
            .unwrap();
        assert_eq!(d.versions[0].version, 2);
    }
    #[test]
    fn tenant_and_permissions_are_part_of_projection() {
        let mut l = SyncLedger::default();
        let mut private = object("v1");
        private.permissions = vec!["actor:1".into()];
        let a = l
            .sync("tenant-a", &Fixture(vec![private]), &ConnectorCursor(None))
            .unwrap();
        let b = l
            .sync(
                "tenant-b",
                &Fixture(vec![object("v1")]),
                &ConnectorCursor(None),
            )
            .unwrap();
        assert_ne!(a.versions[0].permissions, b.versions[0].permissions);
    }
    #[test]
    fn deletion_creates_tombstone_version() {
        let mut l = SyncLedger::default();
        l.sync(
            "tenant-a",
            &Fixture(vec![object("v1")]),
            &ConnectorCursor(None),
        )
        .unwrap();
        let mut deleted = object("v1");
        deleted.deleted = true;
        let out = l
            .sync("tenant-a", &Fixture(vec![deleted]), &ConnectorCursor(None))
            .unwrap();
        assert!(out.versions[0].deleted);
        assert_eq!(out.versions[0].version, 2);
    }
}
