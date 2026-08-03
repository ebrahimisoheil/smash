//! Authorization-first, bounded retrieval contracts for Phase E.
//!
//! The implementation is intentionally adapter-free. PostgreSQL and LanceDB
//! adapters may use these contracts, but core owns the eligibility predicate,
//! ranking, packet bounds, provenance, and degraded-mode semantics.
#![forbid(unsafe_code)]

use engrave_contracts::{AreaId, MemoryId, TenantId};
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;

const BM25_K1: f32 = 1.2;
const BM25_B: f32 = 0.75;
const RRF_K: u32 = 60;
const MAX_CANDIDATES: usize = 2_000;
const DEFAULT_ENTRY_K: usize = 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorRole {
    NormalUser,
    EnterpriseAdmin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Area,
    Enterprise,
    Private,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationContext {
    pub tenant_id: TenantId,
    pub permitted_area_ids: BTreeSet<AreaId>,
    pub role: ActorRole,
    pub purpose: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRequest {
    pub authorization: AuthorizationContext,
    pub query: String,
    pub now: OffsetDateTime,
    pub token_budget: usize,
    pub entry_limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRecord {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub memory_id: MemoryId,
    pub claim: String,
    pub reason: String,
    pub evidence: Vec<String>,
    pub visibility: Visibility,
    pub approved: bool,
    pub current: bool,
    pub archived: bool,
    pub superseded: bool,
    pub expired: bool,
    pub valid_from: Option<OffsetDateTime>,
    pub valid_until: Option<OffsetDateTime>,
    pub applies_when: String,
    pub contradiction_warning: Option<String>,
    pub lineage_warning: Option<String>,
}

impl MemoryRecord {
    fn eligible(&self, request: &SearchRequest) -> bool {
        if self.tenant_id != request.authorization.tenant_id
            || !request
                .authorization
                .permitted_area_ids
                .contains(&self.area_id)
            || !self.approved
            || !self.current
            || self.archived
            || self.superseded
            || self.expired
            || self
                .valid_from
                .is_some_and(|valid_from| request.now < valid_from)
            || self
                .valid_until
                .is_some_and(|valid_until| request.now > valid_until)
            || !applicability_matches(&self.applies_when, &request.authorization.purpose)
        {
            return false;
        }
        match self.visibility {
            Visibility::Private => false,
            Visibility::Area => true,
            Visibility::Enterprise => request.authorization.role == ActorRole::EnterpriseAdmin,
        }
    }
}

fn applicability_matches(rule: &str, purpose: &str) -> bool {
    let normalized = rule.trim().to_ascii_lowercase();
    normalized.is_empty()
        || normalized == "always"
        || normalized == purpose.trim().to_ascii_lowercase()
}

#[derive(Clone, Debug, PartialEq)]
pub struct LexicalHit {
    pub record: MemoryRecord,
    pub score: f32,
    pub rank: usize,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct LexicalIndex {
    records: Vec<MemoryRecord>,
}

impl LexicalIndex {
    pub fn insert(&mut self, record: MemoryRecord) {
        self.records.push(record);
    }

    /// Canonical lexical entry point. Eligibility is evaluated before scoring,
    /// so unauthorized records never become candidates.
    pub fn search(&self, request: &SearchRequest) -> Vec<LexicalHit> {
        let terms = tokenize(&request.query);
        if terms.is_empty() {
            return Vec::new();
        }
        let eligible: Vec<&MemoryRecord> = self
            .records
            .iter()
            .filter(|record| record.eligible(request))
            .collect();
        let average_length = eligible
            .iter()
            .map(|record| tokenize(&record.claim).len().max(1) as f32)
            .sum::<f32>()
            / eligible.len().max(1) as f32;
        let total = eligible.len() as f32;
        let mut scored: Vec<LexicalHit> = eligible
            .into_iter()
            .filter_map(|record| {
                let document_terms = tokenize(&record.claim);
                let document_length = document_terms.len().max(1) as f32;
                let score = terms
                    .iter()
                    .map(|term| {
                        let frequency =
                            document_terms.iter().filter(|item| *item == term).count() as f32;
                        if frequency == 0.0 {
                            return 0.0;
                        }
                        let document_frequency = self
                            .records
                            .iter()
                            .filter(|candidate| {
                                candidate.eligible(request)
                                    && tokenize(&candidate.claim).contains(term)
                            })
                            .count() as f32;
                        let idf = ((total - document_frequency + 0.5) / (document_frequency + 0.5)
                            + 1.0)
                            .ln();
                        idf * (frequency * (BM25_K1 + 1.0))
                            / (frequency
                                + BM25_K1
                                    * (1.0 - BM25_B
                                        + BM25_B * document_length / average_length.max(1.0)))
                    })
                    .sum::<f32>();
                (score > 0.0).then(|| LexicalHit {
                    record: record.clone(),
                    score,
                    rank: 0,
                    reason: "lexical BM25 term match".into(),
                })
            })
            .collect();
        scored.sort_by(|left, right| {
            right.score.total_cmp(&left.score).then_with(|| {
                left.record
                    .memory_id
                    .as_uuid()
                    .cmp(&right.record.memory_id.as_uuid())
            })
        });
        let floor = scored.first().map_or(0.0, |hit| hit.score * 0.35);
        scored.retain(|hit| hit.score >= floor);
        scored.truncate(request.entry_limit.min(DEFAULT_ENTRY_K).min(MAX_CANDIDATES));
        for (index, hit) in scored.iter_mut().enumerate() {
            hit.rank = index + 1;
        }
        scored
    }
}

fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| term.len() > 2)
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionIdentity {
    pub provider: String,
    pub model: String,
    pub model_version: String,
    pub dimension: usize,
    pub projection_version: String,
    pub configuration_fingerprint: String,
}

impl ProjectionIdentity {
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        model_version: impl Into<String>,
        dimension: usize,
        projection_version: impl Into<String>,
        configuration_fingerprint: impl Into<String>,
    ) -> Result<Self, RetrievalError> {
        if dimension == 0 {
            return Err(RetrievalError::InvalidProjection(
                "dimension must be positive",
            ));
        }
        Ok(Self {
            provider: provider.into(),
            model: model.into(),
            model_version: model_version.into(),
            dimension,
            projection_version: projection_version.into(),
            configuration_fingerprint: configuration_fingerprint.into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddingVector {
    pub values: Vec<f32>,
}

impl EmbeddingVector {
    pub fn normalized(values: Vec<f32>, dimension: usize) -> Result<Self, RetrievalError> {
        if values.len() != dimension {
            return Err(RetrievalError::DimensionMismatch {
                expected: dimension,
                actual: values.len(),
            });
        }
        let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
        if !norm.is_finite() || norm <= f32::EPSILON {
            return Err(RetrievalError::InvalidVector);
        }
        Ok(Self {
            values: values.into_iter().map(|value| value / norm).collect(),
        })
    }

    pub fn cosine_similarity(&self, other: &Self) -> Result<f32, RetrievalError> {
        if self.values.len() != other.values.len() {
            return Err(RetrievalError::DimensionMismatch {
                expected: self.values.len(),
                actual: other.values.len(),
            });
        }
        Ok(self
            .values
            .iter()
            .zip(&other.values)
            .map(|(left, right)| left * right)
            .sum())
    }
}

pub trait EmbeddingProvider {
    fn identity(&self) -> &ProjectionIdentity;
    fn embed(&self, input: &str) -> Result<EmbeddingVector, RetrievalError>;
}

#[derive(Clone, Debug)]
pub struct DeterministicEmbeddingProvider {
    identity: ProjectionIdentity,
}

impl DeterministicEmbeddingProvider {
    pub fn new(identity: ProjectionIdentity) -> Self {
        Self { identity }
    }
}

impl EmbeddingProvider for DeterministicEmbeddingProvider {
    fn identity(&self) -> &ProjectionIdentity {
        &self.identity
    }

    fn embed(&self, input: &str) -> Result<EmbeddingVector, RetrievalError> {
        let mut values = vec![0.0; self.identity.dimension];
        for (index, term) in tokenize(input).iter().enumerate() {
            let slot = index % values.len();
            values[slot] += (term.bytes().map(f32::from).sum::<f32>() % 97.0) + 1.0;
        }
        EmbeddingVector::normalized(values, self.identity.dimension)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DenseHit {
    pub record: MemoryRecord,
    pub similarity: f32,
    pub rank: usize,
    pub identity: ProjectionIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingJob {
    pub operation_id: String,
    pub state: JobState,
    pub next_index: usize,
    pub total: usize,
    pub identity: ProjectionIdentity,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectionStore {
    canonical: BTreeMap<MemoryId, MemoryRecord>,
    vectors: BTreeMap<MemoryId, (ProjectionIdentity, EmbeddingVector)>,
}

impl ProjectionStore {
    pub fn add_canonical(&mut self, record: MemoryRecord) {
        self.canonical.insert(record.memory_id, record);
    }

    pub fn add_vector(
        &mut self,
        memory_id: MemoryId,
        identity: ProjectionIdentity,
        vector: EmbeddingVector,
    ) -> Result<(), RetrievalError> {
        if vector.values.len() != identity.dimension {
            return Err(RetrievalError::DimensionMismatch {
                expected: identity.dimension,
                actual: vector.values.len(),
            });
        }
        self.vectors.insert(memory_id, (identity, vector));
        Ok(())
    }

    pub fn remove_vector(&mut self, memory_id: MemoryId) {
        self.vectors.remove(&memory_id);
    }

    pub fn corrupt_vector(&mut self, memory_id: MemoryId, vector: EmbeddingVector) {
        if let Some((identity, _)) = self.vectors.get(&memory_id).cloned() {
            self.vectors.insert(memory_id, (identity, vector));
        }
    }

    pub fn reconcile(&mut self, identity: &ProjectionIdentity) -> ReconciliationReport {
        let mut report = ReconciliationReport::default();
        let ids: Vec<MemoryId> = self.canonical.keys().copied().collect();
        for memory_id in ids {
            let valid = self.vectors.get(&memory_id).is_some_and(|(found, vector)| {
                found == identity && vector.values.len() == identity.dimension
            });
            if valid {
                report.verified += 1;
            } else if let Some(record) = self.canonical.get(&memory_id) {
                let vector = deterministic_vector(record, identity.dimension);
                self.vectors.insert(memory_id, (identity.clone(), vector));
                report.repaired += 1;
            }
        }
        let stale: Vec<MemoryId> = self
            .vectors
            .keys()
            .filter(|memory_id| !self.canonical.contains_key(memory_id))
            .copied()
            .collect();
        report.removed = stale.len();
        for memory_id in stale {
            self.vectors.remove(&memory_id);
        }
        report
    }

    pub fn dense_search(
        &self,
        request: &SearchRequest,
        query: &EmbeddingVector,
        identity: &ProjectionIdentity,
    ) -> Vec<DenseHit> {
        let mut hits: Vec<DenseHit> = self
            .canonical
            .values()
            .filter(|record| record.eligible(request))
            .filter_map(|record| {
                let (found_identity, vector) = self.vectors.get(&record.memory_id)?;
                if found_identity != identity {
                    return None;
                }
                Some(DenseHit {
                    record: record.clone(),
                    similarity: query.cosine_similarity(vector).ok()?,
                    rank: 0,
                    identity: found_identity.clone(),
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            right.similarity.total_cmp(&left.similarity).then_with(|| {
                left.record
                    .memory_id
                    .as_uuid()
                    .cmp(&right.record.memory_id.as_uuid())
            })
        });
        hits.truncate(request.entry_limit.min(DEFAULT_ENTRY_K));
        for (index, hit) in hits.iter_mut().enumerate() {
            hit.rank = index + 1;
        }
        hits
    }
}

fn deterministic_vector(record: &MemoryRecord, dimension: usize) -> EmbeddingVector {
    let mut values = vec![0.0; dimension];
    for (index, term) in tokenize(&record.claim).iter().enumerate() {
        let slot = index % dimension;
        values[slot] += term.bytes().map(f32::from).sum::<f32>() + 1.0;
    }
    EmbeddingVector::normalized(values, dimension).expect("positive deterministic vector")
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReconciliationReport {
    pub verified: usize,
    pub repaired: usize,
    pub removed: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FusionMode {
    Rrf,
    Weighted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchProfile {
    pub mode: FusionMode,
    pub alpha: u8,
    pub rrf_k: u32,
    pub rerank_head: usize,
}

impl Default for SearchProfile {
    fn default() -> Self {
        Self {
            mode: FusionMode::Rrf,
            alpha: 90,
            rrf_k: RRF_K,
            rerank_head: 30,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DegradedMode {
    None,
    LexicalOnly { reason: String },
    DenseProjectionStale { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrievalResult {
    pub memory_id: MemoryId,
    pub area_id: AreaId,
    pub claim: String,
    pub reason: String,
    pub provenance: Vec<String>,
    pub applicability: String,
    pub warnings: Vec<String>,
    pub estimated_tokens: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchTrace {
    pub lexical_candidates: usize,
    pub dense_candidates: usize,
    pub fused_candidates: usize,
    pub returned_results: usize,
    pub bounded_cap: usize,
    pub token_budget: usize,
    pub degraded_mode: DegradedMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrievalPacket {
    pub results: Vec<RetrievalResult>,
    pub tokens_used: usize,
    pub trace: SearchTrace,
}

pub fn light_search(
    request: &SearchRequest,
    lexical: &[LexicalHit],
    dense: &[DenseHit],
    profile: &SearchProfile,
    degraded_mode: DegradedMode,
) -> RetrievalPacket {
    let mut lexical_ranks = BTreeMap::new();
    for hit in lexical {
        lexical_ranks.insert(hit.record.memory_id, hit.rank);
    }
    let mut dense_ranks = BTreeMap::new();
    for hit in dense {
        dense_ranks.insert(hit.record.memory_id, hit.rank);
    }
    let records: BTreeMap<MemoryId, MemoryRecord> = lexical
        .iter()
        .map(|hit| (hit.record.memory_id, hit.record.clone()))
        .chain(
            dense
                .iter()
                .map(|hit| (hit.record.memory_id, hit.record.clone())),
        )
        .collect();
    let mut fused: Vec<(MemoryId, f32)> = records
        .keys()
        .copied()
        .map(|memory_id| {
            let score = match profile.mode {
                FusionMode::Rrf => {
                    lexical_ranks
                        .get(&memory_id)
                        .map_or(0.0, |rank| 1.0 / (profile.rrf_k + *rank as u32) as f32)
                        + dense_ranks
                            .get(&memory_id)
                            .map_or(0.0, |rank| 1.0 / (profile.rrf_k + *rank as u32) as f32)
                }
                FusionMode::Weighted => {
                    let dense_score = dense
                        .iter()
                        .find(|hit| hit.record.memory_id == memory_id)
                        .map_or(0.0, |hit| hit.similarity);
                    let lexical_score = lexical
                        .iter()
                        .find(|hit| hit.record.memory_id == memory_id)
                        .map_or(0.0, |hit| hit.score);
                    (profile.alpha as f32 / 100.0) * dense_score
                        + (1.0 - profile.alpha as f32 / 100.0) * lexical_score
                }
            };
            (memory_id, score)
        })
        .collect();
    fused.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.as_uuid().cmp(&right.0.as_uuid()))
    });
    fused.truncate(request.entry_limit.min(MAX_CANDIDATES));
    let mut results = Vec::new();
    let mut tokens_used = 0;
    for (memory_id, _) in &fused {
        let record = &records[memory_id];
        let estimated_tokens = estimate_tokens(record);
        if tokens_used + estimated_tokens > request.token_budget {
            break;
        }
        let mut warnings = Vec::new();
        if let Some(warning) = &record.contradiction_warning {
            warnings.push(warning.clone());
        }
        if let Some(warning) = &record.lineage_warning {
            warnings.push(warning.clone());
        }
        let reason = match (
            lexical_ranks.contains_key(memory_id),
            dense_ranks.contains_key(memory_id),
        ) {
            (true, true) => "selected by lexical+dense fusion",
            (true, false) => "selected by lexical retrieval",
            (false, true) => "selected by dense retrieval",
            (false, false) => "selected by bounded ranking",
        };
        results.push(RetrievalResult {
            memory_id: *memory_id,
            area_id: record.area_id,
            claim: record.claim.clone(),
            reason: reason.into(),
            provenance: record.evidence.clone(),
            applicability: record.applies_when.clone(),
            warnings,
            estimated_tokens,
        });
        tokens_used += estimated_tokens;
    }
    RetrievalPacket {
        results: results.clone(),
        tokens_used,
        trace: SearchTrace {
            lexical_candidates: lexical.len(),
            dense_candidates: dense.len(),
            fused_candidates: fused.len(),
            returned_results: results.len(),
            bounded_cap: request.entry_limit.min(MAX_CANDIDATES),
            token_budget: request.token_budget,
            degraded_mode,
        },
    }
}

fn estimate_tokens(record: &MemoryRecord) -> usize {
    (record.claim.split_whitespace().count()
        + record.reason.split_whitespace().count()
        + record
            .evidence
            .iter()
            .map(|item| item.split_whitespace().count())
            .sum::<usize>()
        + 3)
    .max(1)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetrievalError {
    InvalidProjection(&'static str),
    DimensionMismatch { expected: usize, actual: usize },
    InvalidVector,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BenchmarkMetrics {
    pub queries: usize,
    pub recall_at_5: usize,
    pub recall_at_10: usize,
    pub unauthorized_results: usize,
    pub wrong_area_results: usize,
    pub tokens_used: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenchmarkThresholds {
    pub minimum_recall_at_5: usize,
    pub maximum_unauthorized_results: usize,
    pub maximum_wrong_area_results: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkGateError {
    RecallRegression,
    UnauthorizedResults,
    WrongAreaResults,
}

pub fn enforce_benchmark_gate(
    metrics: &BenchmarkMetrics,
    thresholds: BenchmarkThresholds,
) -> Result<(), BenchmarkGateError> {
    if metrics.recall_at_5 < thresholds.minimum_recall_at_5 {
        return Err(BenchmarkGateError::RecallRegression);
    }
    if metrics.unauthorized_results > thresholds.maximum_unauthorized_results {
        return Err(BenchmarkGateError::UnauthorizedResults);
    }
    if metrics.wrong_area_results > thresholds.maximum_wrong_area_results {
        return Err(BenchmarkGateError::WrongAreaResults);
    }
    Ok(())
}

pub struct BenchmarkCase<'a> {
    pub packet: &'a RetrievalPacket,
    pub gold: &'a BTreeSet<MemoryId>,
    pub excluded: &'a BTreeSet<MemoryId>,
    pub permitted_areas: &'a BTreeSet<AreaId>,
}

pub fn evaluate_benchmark(cases: &[BenchmarkCase<'_>]) -> BenchmarkMetrics {
    let mut metrics = BenchmarkMetrics {
        queries: cases.len(),
        ..BenchmarkMetrics::default()
    };
    for case in cases {
        let packet = case.packet;
        let gold = case.gold;
        let excluded = case.excluded;
        let permitted_areas = case.permitted_areas;
        let ids: Vec<_> = packet
            .results
            .iter()
            .map(|result| result.memory_id)
            .collect();
        if ids.iter().take(5).any(|id| gold.contains(id)) {
            metrics.recall_at_5 += 1;
        }
        if ids.iter().take(10).any(|id| gold.contains(id)) {
            metrics.recall_at_10 += 1;
        }
        metrics.unauthorized_results += ids.iter().filter(|id| excluded.contains(id)).count();
        metrics.wrong_area_results += packet
            .results
            .iter()
            .filter(|result| !permitted_areas.contains(&result.area_id))
            .count();
        metrics.tokens_used += packet.tokens_used;
    }
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use engrave_contracts::{AreaId, MemoryId, TenantId};
    use std::collections::BTreeSet;

    fn ids() -> (TenantId, AreaId, AreaId, MemoryId, MemoryId, MemoryId) {
        (
            TenantId::new_v7(),
            AreaId::new_v7(),
            AreaId::new_v7(),
            MemoryId::new_v7(),
            MemoryId::new_v7(),
            MemoryId::new_v7(),
        )
    }

    fn record(
        tenant_id: TenantId,
        area_id: AreaId,
        memory_id: MemoryId,
        claim: &str,
    ) -> MemoryRecord {
        MemoryRecord {
            tenant_id,
            area_id,
            memory_id,
            claim: claim.into(),
            reason: "approved fixture".into(),
            evidence: vec!["source-version:1#chunk:1".into()],
            visibility: Visibility::Area,
            approved: true,
            current: true,
            archived: false,
            superseded: false,
            expired: false,
            valid_from: None,
            valid_until: None,
            applies_when: "always".into(),
            contradiction_warning: None,
            lineage_warning: None,
        }
    }

    fn request(tenant_id: TenantId, area_id: AreaId, purpose: &str) -> SearchRequest {
        SearchRequest {
            authorization: AuthorizationContext {
                tenant_id,
                permitted_area_ids: BTreeSet::from([area_id]),
                role: ActorRole::NormalUser,
                purpose: purpose.into(),
            },
            query: "executive security renewal".into(),
            now: OffsetDateTime::UNIX_EPOCH,
            token_budget: 100,
            entry_limit: 30,
        }
    }

    #[test]
    fn lexical_prefilter_excludes_wrong_tenant_area_and_lifecycle_records() {
        let (tenant, sales, marketing, eligible, wrong_area, archived) = ids();
        let mut index = LexicalIndex::default();
        index.insert(record(
            tenant,
            sales,
            eligible,
            "Executive security review before renewal",
        ));
        index.insert(record(
            tenant,
            marketing,
            wrong_area,
            "Executive security review before renewal",
        ));
        let mut archived_record = record(
            tenant,
            sales,
            archived,
            "Executive security review before renewal",
        );
        archived_record.archived = true;
        index.insert(archived_record);
        let hits = index.search(&request(tenant, sales, "renewal_preparation"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record.memory_id, eligible);
    }

    #[test]
    fn enterprise_visibility_requires_enterprise_admin() {
        let (tenant, area, _, memory, _, _) = ids();
        let mut index = LexicalIndex::default();
        let mut enterprise = record(tenant, area, memory, "enterprise renewal policy");
        enterprise.visibility = Visibility::Enterprise;
        index.insert(enterprise);
        assert!(index.search(&request(tenant, area, "always")).is_empty());
        let mut admin_request = request(tenant, area, "always");
        admin_request.authorization.role = ActorRole::EnterpriseAdmin;
        assert_eq!(index.search(&admin_request).len(), 1);
    }

    #[test]
    fn provider_identity_and_dimension_prevent_mixed_projections() {
        let identity = ProjectionIdentity::new("test", "unit", "1", 3, "p1", "fp1").unwrap();
        let other = ProjectionIdentity::new("test", "unit", "2", 3, "p2", "fp2").unwrap();
        let provider = DeterministicEmbeddingProvider::new(identity.clone());
        let vector = provider.embed("renewal security").unwrap();
        assert_eq!(vector.values.len(), 3);
        assert!(EmbeddingVector::normalized(vec![1.0, 2.0], 3).is_err());
        let (tenant, area, _, memory, _, _) = ids();
        let mut projections = ProjectionStore::default();
        projections.add_canonical(record(tenant, area, memory, "renewal security"));
        projections
            .add_vector(memory, identity.clone(), vector)
            .unwrap();
        let request = request(tenant, area, "always");
        assert!(projections
            .dense_search(&request, &provider.embed("renewal").unwrap(), &other)
            .is_empty());
    }

    #[test]
    fn reconciliation_repairs_deletion_corruption_and_stale_rows() {
        let identity = ProjectionIdentity::new("test", "unit", "1", 3, "p1", "fp1").unwrap();
        let provider = DeterministicEmbeddingProvider::new(identity.clone());
        let (tenant, area, _, first, second, stale) = ids();
        let mut projections = ProjectionStore::default();
        projections.add_canonical(record(tenant, area, first, "first memory"));
        projections.add_canonical(record(tenant, area, second, "second memory"));
        projections
            .add_vector(first, identity.clone(), provider.embed("first").unwrap())
            .unwrap();
        projections
            .add_vector(stale, identity.clone(), provider.embed("stale").unwrap())
            .unwrap();
        projections.corrupt_vector(first, EmbeddingVector { values: vec![1.0] });
        let report = projections.reconcile(&identity);
        assert_eq!(report.repaired, 2);
        assert_eq!(report.removed, 1);
        assert_eq!(report.verified, 0);
    }

    #[test]
    fn light_search_fuses_explains_and_packs_under_budget() {
        let (tenant, area, _, first, second, _) = ids();
        let first_record = record(
            tenant,
            area,
            first,
            "Executive security review before renewal",
        );
        let mut second_record = record(tenant, area, second, "Renewal requires executive review");
        second_record.lineage_warning = Some("supersedes prior memory".into());
        let lexical = vec![LexicalHit {
            record: first_record.clone(),
            score: 2.0,
            rank: 1,
            reason: "BM25".into(),
        }];
        let identity = ProjectionIdentity::new("test", "unit", "1", 3, "p1", "fp1").unwrap();
        let dense = vec![DenseHit {
            record: second_record,
            similarity: 0.99,
            rank: 1,
            identity,
        }];
        let request = request(tenant, area, "always");
        let packet = light_search(
            &request,
            &lexical,
            &dense,
            &SearchProfile::default(),
            DegradedMode::None,
        );
        assert_eq!(packet.results.len(), 2);
        assert!(packet.tokens_used <= request.token_budget);
        assert!(packet
            .results
            .iter()
            .all(|result| !result.reason.is_empty() && !result.provenance.is_empty()));
        assert!(packet
            .results
            .iter()
            .any(|result| !result.warnings.is_empty()));
        assert_eq!(packet.trace.lexical_candidates, 1);
        assert_eq!(packet.trace.dense_candidates, 1);
    }

    #[test]
    fn lexical_degraded_mode_is_visible_and_metrics_are_deterministic() {
        let (tenant, area, _, memory, _, _) = ids();
        let record = record(tenant, area, memory, "renewal security review");
        let lexical = vec![LexicalHit {
            record,
            score: 1.0,
            rank: 1,
            reason: "BM25".into(),
        }];
        let request = request(tenant, area, "always");
        let packet = light_search(
            &request,
            &lexical,
            &[],
            &SearchProfile::default(),
            DegradedMode::LexicalOnly {
                reason: "embedding key unavailable".into(),
            },
        );
        assert!(matches!(
            packet.trace.degraded_mode,
            DegradedMode::LexicalOnly { .. }
        ));
        let gold = BTreeSet::from([memory]);
        let excluded = BTreeSet::new();
        let areas = BTreeSet::from([area]);
        let metrics = evaluate_benchmark(&[BenchmarkCase {
            packet: &packet,
            gold: &gold,
            excluded: &excluded,
            permitted_areas: &areas,
        }]);
        assert_eq!(metrics.recall_at_5, 1);
        assert_eq!(metrics.unauthorized_results, 0);
        assert_eq!(metrics.wrong_area_results, 0);
        assert!(enforce_benchmark_gate(
            &metrics,
            BenchmarkThresholds {
                minimum_recall_at_5: 1,
                maximum_unauthorized_results: 0,
                maximum_wrong_area_results: 0,
            }
        )
        .is_ok());
        assert_eq!(
            enforce_benchmark_gate(
                &metrics,
                BenchmarkThresholds {
                    minimum_recall_at_5: 2,
                    maximum_unauthorized_results: 0,
                    maximum_wrong_area_results: 0,
                }
            ),
            Err(BenchmarkGateError::RecallRegression)
        );
    }
}
