#![allow(dead_code)]

use engrave_contracts::{Origin, ProposalState, RuleEffect, SourceState};
use serde::Deserialize;

const FIXTURE: &str = include_str!("../../../eval/fixtures/sales/fixture.toml");
const COVERAGE: &str = include_str!("../../../eval/fixtures/sales/coverage.md");
const BENCHMARK: &str = include_str!("../../../eval/fixtures/sales/benchmark.toml");

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

#[derive(Debug, Deserialize)]
struct Benchmark {
    meta: BenchmarkMeta,
    eligibility: Eligibility,
    queries: Vec<Query>,
    profiles: Vec<Profile>,
    exact_vector_reference: ExactVectorReference,
}

#[derive(Debug, Deserialize)]
struct BenchmarkMeta {
    benchmark_id: String,
    fixture_id: String,
    schema_version: u8,
    tenant_id: String,
    default_area_id: String,
}

#[derive(Debug, Deserialize)]
struct Eligibility {
    eligible_memory_ids: Vec<String>,
    excluded_memory_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Query {
    id: String,
    text: String,
    area_id: String,
    purpose: String,
    gold_memory_ids: Vec<String>,
    excluded_memory_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Profile {
    name: String,
    channels: Vec<String>,
    fusion: String,
    #[serde(default)]
    rrf_k: Option<u16>,
    #[serde(default)]
    alpha: Option<f32>,
    #[serde(default)]
    rerank_head: Option<u16>,
    degraded_mode: String,
}

#[derive(Debug, Deserialize)]
struct ExactVectorReference {
    dimension: usize,
    distance: String,
    normalization: String,
    query_id: String,
    expected_rank: Vec<String>,
    vectors: Vec<VectorFixture>,
}

#[derive(Debug, Deserialize)]
struct VectorFixture {
    id: String,
    values: Vec<f32>,
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

#[test]
fn phase_e_benchmark_manifest_is_deterministic_and_authorization_first() {
    let benchmark: Benchmark = toml::from_str(BENCHMARK).expect("benchmark TOML must parse");
    assert_eq!(benchmark.meta.benchmark_id, "sales-phase-e-v1");
    assert_eq!(benchmark.meta.fixture_id, "sales-phase-a-v1");
    assert_eq!(benchmark.meta.schema_version, 1);
    assert_eq!(
        benchmark.meta.tenant_id,
        "018f0000-0000-7000-8000-000000000001"
    );
    assert_eq!(
        benchmark.meta.default_area_id,
        "018f0000-0000-7000-8000-000000000010"
    );
    assert_eq!(benchmark.eligibility.eligible_memory_ids.len(), 1);
    assert_eq!(benchmark.eligibility.excluded_memory_ids.len(), 2);
    assert_eq!(benchmark.queries.len(), 3);
    assert!(benchmark.queries.iter().all(|query| !query.text.is_empty()));
    assert!(benchmark
        .queries
        .iter()
        .all(|query| !query.purpose.is_empty()));
    assert!(benchmark.queries.iter().all(|query| {
        query
            .gold_memory_ids
            .iter()
            .all(|id| benchmark.eligibility.eligible_memory_ids.contains(id))
    }));
    let wrong_area = benchmark
        .queries
        .iter()
        .find(|query| query.id == "wrong-area-marketing")
        .expect("wrong-area query");
    assert!(wrong_area.gold_memory_ids.is_empty());
    assert_eq!(wrong_area.excluded_memory_ids.len(), 3);

    let profile_names: Vec<_> = benchmark
        .profiles
        .iter()
        .map(|profile| profile.name.as_str())
        .collect();
    assert_eq!(
        profile_names,
        vec![
            "lexical-only",
            "exact-dense",
            "rrf-baseline",
            "weighted-comparison",
            "rerank-comparison",
        ]
    );
    let rrf = benchmark
        .profiles
        .iter()
        .find(|profile| profile.name == "rrf-baseline")
        .expect("RRF profile");
    assert_eq!(rrf.rrf_k, Some(60));
    let weighted = benchmark
        .profiles
        .iter()
        .find(|profile| profile.name == "weighted-comparison")
        .expect("weighted profile");
    assert_eq!(weighted.alpha, Some(0.9));
    let rerank = benchmark
        .profiles
        .iter()
        .find(|profile| profile.name == "rerank-comparison")
        .expect("rerank profile");
    assert_eq!(rerank.rerank_head, Some(30));
    assert!(benchmark
        .profiles
        .iter()
        .any(|profile| profile.degraded_mode == "lexical_only"));
    assert!(benchmark
        .profiles
        .iter()
        .all(|profile| !profile.channels.is_empty() && !profile.fusion.is_empty()));

    let reference = benchmark.exact_vector_reference;
    assert_eq!(reference.dimension, 3);
    assert_eq!(reference.distance, "cosine");
    assert_eq!(reference.normalization, "l2");
    assert_eq!(reference.query_id, "renewal-security-signoff");
    assert_eq!(reference.vectors.len(), reference.expected_rank.len());
    assert!(reference
        .vectors
        .iter()
        .all(|vector| vector.values.len() == reference.dimension));

    let query = [1.0_f32, 0.0, 0.0];
    let mut scored: Vec<_> = reference
        .vectors
        .iter()
        .map(|vector| {
            let norm = vector
                .values
                .iter()
                .map(|value| value * value)
                .sum::<f32>()
                .sqrt();
            let similarity = vector
                .values
                .iter()
                .zip(query)
                .map(|(value, query_value)| value * query_value)
                .sum::<f32>()
                / norm;
            (vector.id.as_str(), similarity)
        })
        .collect();
    scored.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .expect("fixture vectors must be finite")
            .then_with(|| left.0.cmp(right.0))
    });
    let exact_rank: Vec<_> = scored.into_iter().map(|(id, _)| id).collect();
    assert_eq!(
        exact_rank,
        reference
            .expected_rank
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
}
