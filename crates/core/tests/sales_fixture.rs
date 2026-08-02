#![allow(dead_code)]

use serde::Deserialize;
use smash_contracts::{Origin, ProposalState, RuleEffect, SourceState};

const FIXTURE: &str = include_str!("../../../eval/fixtures/sales/fixture.toml");
const COVERAGE: &str = include_str!("../../../eval/fixtures/sales/coverage.md");

#[derive(Debug, Deserialize)]
struct Fixture {
    meta: Meta,
    areas: Vec<Area>,
    accounts: Vec<Record>,
    people: Vec<Record>,
    opportunity: Record,
    sources: Vec<Source>,
    chunks: Vec<Chunk>,
    memories: Vec<Memory>,
    proposals: Vec<Proposal>,
    rule: Rule,
    cross_map: CrossMap,
    events: Vec<Event>,
    replay: Replay,
    concurrency: Concurrency,
}

#[derive(Debug, Deserialize)]
struct Meta {
    fixture_id: String,
    schema_version: u8,
    tenant_id: String,
    created_at: String,
}
#[derive(Debug, Deserialize)]
struct Area {
    id: String,
    slug: String,
    map_version_id: String,
}
#[derive(Debug, Deserialize)]
struct Record {
    id: String,
    name: Option<String>,
    origin: Option<Origin>,
    #[serde(flatten)]
    extra: toml::Table,
}
#[derive(Debug, Deserialize)]
struct Source {
    id: String,
    kind: String,
    state: SourceState,
    versions: Vec<SourceVersion>,
}
#[derive(Debug, Deserialize)]
struct SourceVersion {
    id: String,
    version: u8,
    state: String,
    checksum: String,
}
#[derive(Debug, Deserialize)]
struct Chunk {
    id: String,
    source_version_id: String,
    representation: String,
    coordinate: String,
    content_hash: String,
}
#[derive(Debug, Deserialize)]
struct Memory {
    id: String,
    state: String,
    origin: Origin,
    claim: String,
    #[serde(default)]
    evidence_source_version_ids: Vec<String>,
    #[serde(default)]
    contradicts_memory_id: Option<String>,
    #[serde(default)]
    supersedes_memory_id: Option<String>,
    #[serde(default)]
    supersession_reason: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Proposal {
    id: String,
    state: ProposalState,
    origin: Origin,
    kind: String,
    #[serde(default)]
    rejection_reason: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Rule {
    id: String,
    area_id: String,
    state: String,
    cases: Vec<RuleCase>,
}
#[derive(Debug, Deserialize)]
struct RuleCase {
    name: String,
    effect: RuleEffect,
}
#[derive(Debug, Deserialize)]
struct CrossMap {
    id: String,
    state: String,
    source_area_id: String,
    target_area_id: String,
    relation: String,
    rationale: String,
}
#[derive(Debug, Deserialize)]
struct Event {
    action: String,
    target_type: String,
    target_id: String,
    idempotency_key: String,
}
#[derive(Debug, Deserialize)]
struct Replay {
    idempotency_key: String,
    first_result: String,
    replayed_result: String,
}
#[derive(Debug, Deserialize)]
struct Concurrency {
    resource_id: String,
    reviewed_version: u8,
    current_version: u8,
    result: String,
}

#[test]
fn canonical_sales_fixture_is_deterministic_and_complete() {
    let fixture: Fixture = toml::from_str(FIXTURE).expect("fixture TOML must parse");
    assert_eq!(fixture.meta.fixture_id, "sales-phase-a-v1");
    assert_eq!(fixture.meta.schema_version, 1);
    assert_eq!(fixture.meta.created_at, "2026-01-15T09:00:00Z");
    assert_eq!(fixture.areas.len(), 2);
    assert_eq!(fixture.accounts.len(), 2);
    assert_eq!(fixture.people.len(), 4);
    assert_eq!(
        fixture.opportunity.id,
        "018f0000-0000-7000-8000-000000000120"
    );
    assert_eq!(fixture.sources.len(), 2);
    assert_eq!(fixture.sources[0].state, SourceState::Ready);
    assert_eq!(fixture.sources[1].versions.len(), 2);
    assert_eq!(fixture.chunks.len(), 2);
    assert_eq!(fixture.memories.len(), 3);
    assert_eq!(fixture.memories[0].origin, Origin::Approved);
    assert_eq!(fixture.memories[0].evidence_source_version_ids.len(), 2);
    assert!(fixture
        .memories
        .iter()
        .any(|m| m.contradicts_memory_id.is_some()));
    assert!(fixture
        .memories
        .iter()
        .any(|m| m.supersession_reason.is_some()));
    assert_eq!(fixture.proposals.len(), 3);
    assert!(fixture
        .proposals
        .iter()
        .any(|p| p.state == ProposalState::Rejected && p.rejection_reason.is_some()));
    assert_eq!(
        fixture
            .rule
            .cases
            .iter()
            .map(|c| c.effect)
            .collect::<Vec<_>>(),
        vec![RuleEffect::Block, RuleEffect::RequireApproval]
    );
    assert_eq!(fixture.cross_map.state, "proposed");
    assert!(fixture.events.len() >= 3);
    assert_eq!(fixture.replay.first_result, fixture.replay.replayed_result);
    assert_eq!(fixture.concurrency.result, "resource.version_conflict");
    assert_eq!(
        COVERAGE
            .lines()
            .filter(|line| line.starts_with('|'))
            .count()
            - 2,
        13
    );
}
