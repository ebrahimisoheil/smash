//! Credential-free deterministic Phase G fixture and benchmark generator.
//! Authorization labels, lifecycle, IDs, decoys, and expected outcomes are
//! generated here; no model is involved in the security oracle.
#![forbid(unsafe_code)]

use crate::rules::*;
use engrave_contracts::{AreaId, RuleEffect, RuleId, RuleVersionId, TenantId};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureConfig {
    pub tenants: usize,
    pub areas_per_tenant: usize,
    pub personas: usize,
    pub sources: usize,
    pub memories: usize,
    pub seed: u64,
}

impl Default for FixtureConfig {
    fn default() -> Self {
        Self {
            tenants: 2,
            areas_per_tenant: 3,
            personas: 5,
            sources: 50,
            memories: 500,
            seed: 7,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureObject {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub kind: &'static str,
    pub lifecycle: &'static str,
    pub sensitivity: &'static str,
    pub decoy: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureExpected {
    pub effect: RuleEffect,
    pub rationale: String,
    pub next_action: String,
    pub envelope: PolicyEnvelope,
    pub audit_outcome: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureCorpus {
    pub config: FixtureConfig,
    pub tenants: Vec<TenantId>,
    pub areas: Vec<AreaId>,
    pub personas: Vec<Uuid>,
    pub objects: Vec<FixtureObject>,
    pub expected: Vec<FixtureExpected>,
    pub benchmark_metadata: BenchmarkMetadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkMetadata {
    pub seed: u64,
    pub generated_at: String,
    pub region: String,
    pub corpus_size: usize,
    pub concurrency: usize,
    pub hardware: String,
}

fn id(seed: u64, kind: u8, index: usize) -> Uuid {
    let mut h = Sha256::new();
    h.update(seed.to_le_bytes());
    h.update([kind]);
    h.update((index as u64).to_le_bytes());
    let digest = h.finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

pub fn generate(config: FixtureConfig) -> FixtureCorpus {
    let tenants = (0..config.tenants)
        .map(|i| TenantId::new(id(config.seed, 1, i)))
        .collect::<Vec<_>>();
    let areas = (0..config.tenants * config.areas_per_tenant)
        .map(|i| AreaId::new(id(config.seed, 2, i)))
        .collect::<Vec<_>>();
    let personas = (0..config.personas)
        .map(|i| id(config.seed, 3, i))
        .collect::<Vec<_>>();
    let mut objects = Vec::with_capacity(config.sources + config.memories);
    for i in 0..config.sources + config.memories {
        let tenant = tenants[i % tenants.len()];
        let area = areas[i % areas.len()];
        let kind = if i < config.sources {
            "source"
        } else {
            "memory"
        };
        let decoy = i % 11 == 0;
        let lifecycle = match i % 7 {
            0 => "current",
            1 => "stale",
            2 => "superseded",
            3 => "private",
            4 => "pending",
            5 => "expired",
            _ => "ready",
        };
        objects.push(FixtureObject {
            id: id(config.seed, 4, i),
            tenant_id: tenant,
            area_id: area,
            kind,
            lifecycle,
            sensitivity: if i % 5 == 0 { "private" } else { "normal" },
            decoy,
        });
    }
    let expected = areas
        .iter()
        .enumerate()
        .map(|(i, area)| {
            let envelope = PolicyEnvelope {
                version: POLICY_ENVELOPE_VERSION.into(),
                allowed_area_ids: BTreeSet::from([*area]),
                allowed_object_types: BTreeSet::from([ObjectType::Memory, ObjectType::Source]),
                allowed_fields: BTreeSet::from(["claim".into(), "provenance".into()]),
                blocked_actions: BTreeSet::new(),
                approval_requirements: BTreeSet::new(),
                rule_ids: vec![(
                    RuleId::new(id(config.seed, 5, i)),
                    RuleVersionId::new(id(config.seed, 6, i)),
                )],
            };
            FixtureExpected {
                effect: if i % 4 == 0 {
                    RuleEffect::Block
                } else if i % 4 == 1 {
                    RuleEffect::RequireApproval
                } else if i % 4 == 2 {
                    RuleEffect::Warn
                } else {
                    RuleEffect::Allow
                },
                rationale: "deterministic fixture policy".into(),
                next_action: "fixture_oracle".into(),
                envelope,
                audit_outcome: "recorded".into(),
            }
        })
        .collect();
    FixtureCorpus {
        config: config.clone(),
        tenants,
        areas,
        personas,
        objects,
        expected,
        benchmark_metadata: BenchmarkMetadata {
            seed: config.seed,
            generated_at: "fixture-seed-time".into(),
            region: "local".into(),
            corpus_size: config.sources + config.memories,
            concurrency: 1,
            hardware: "fixture".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_seed_is_byte_stable() {
        assert_eq!(
            generate(FixtureConfig::default()),
            generate(FixtureConfig::default())
        );
    }
    #[test]
    fn decoys_are_deterministic_and_credential_free() {
        let c = generate(FixtureConfig {
            sources: 50,
            memories: 500,
            ..Default::default()
        });
        assert_eq!(c.objects.len(), 550);
        assert!(c.objects.iter().any(|o| o.decoy));
        assert_eq!(c.benchmark_metadata.generated_at, "fixture-seed-time");
    }
}
