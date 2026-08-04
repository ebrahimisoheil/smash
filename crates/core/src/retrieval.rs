//! Authorization-first, bounded retrieval contracts for Phase E.
//!
//! The implementation is intentionally adapter-free. PostgreSQL and LanceDB
//! adapters may use these contracts, but core owns the eligibility predicate,
//! ranking, packet bounds, provenance, and degraded-mode semantics.
#![forbid(unsafe_code)]

use crate::cross_map;
use crate::rules::PolicyEnvelope;
use engrave_contracts::{AreaId, CrossMapMapping, CrossMapRelation, MemoryId, TenantId};
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;
use uuid::Uuid;

pub const PRODUCTION_OUTPUT_DIMENSION: usize = 1024;

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
    /// The authenticated human or agent inside the active tenant. `None`
    /// means there is no principal eligible to read private records.
    pub actor_id: Option<Uuid>,
    pub permitted_area_ids: BTreeSet<AreaId>,
    pub role: ActorRole,
    pub purpose: String,
}

/// Applies the mechanically-produced policy envelope before either dense or
/// lexical retrieval. Both retrieval paths receive the same narrowed scope.
pub fn apply_policy_envelope(authorization: &mut AuthorizationContext, envelope: &PolicyEnvelope) {
    authorization
        .permitted_area_ids
        .retain(|area| envelope.allowed_area_ids.contains(area));
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
    pub owner_actor_id: Option<Uuid>,
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
            Visibility::Private => request
                .authorization
                .actor_id
                .is_some_and(|actor_id| self.owner_actor_id == Some(actor_id)),
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
    /// Native dimension emitted by the provider before projection.
    pub native_dimension: usize,
    /// Dimension stored and searched by ENGRAVE.
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
            native_dimension: dimension,
            dimension,
            projection_version: projection_version.into(),
            configuration_fingerprint: configuration_fingerprint.into(),
        })
    }

    pub fn with_native_dimension(
        provider: impl Into<String>,
        model: impl Into<String>,
        model_version: impl Into<String>,
        native_dimension: usize,
        output_dimension: usize,
        projection_version: impl Into<String>,
        configuration_fingerprint: impl Into<String>,
    ) -> Result<Self, RetrievalError> {
        if native_dimension == 0 || output_dimension == 0 {
            return Err(RetrievalError::InvalidProjection(
                "dimensions must be positive",
            ));
        }
        Ok(Self {
            provider: provider.into(),
            model: model.into(),
            model_version: model_version.into(),
            native_dimension,
            dimension: output_dimension,
            projection_version: projection_version.into(),
            configuration_fingerprint: configuration_fingerprint.into(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingProfile {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub model_version: String,
    pub native_dimension: usize,
    pub output_dimension: usize,
    pub projection_version: String,
    pub configuration_fingerprint: String,
    pub production: bool,
    pub credential_env: Option<String>,
}

impl EmbeddingProfile {
    pub fn identity(&self) -> Result<ProjectionIdentity, RetrievalError> {
        ProjectionIdentity::with_native_dimension(
            self.provider.clone(),
            self.model.clone(),
            self.model_version.clone(),
            self.native_dimension,
            self.output_dimension,
            self.projection_version.clone(),
            self.configuration_fingerprint.clone(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigurationError {
    MissingProfile,
    DuplicateProfile,
    InvalidProfile(String),
    MissingCredential(String),
}

#[derive(Clone, Debug, Default)]
pub struct EmbeddingConfiguration {
    profiles: BTreeMap<String, EmbeddingProfile>,
}

impl EmbeddingConfiguration {
    pub fn new(
        profiles: impl IntoIterator<Item = EmbeddingProfile>,
    ) -> Result<Self, ConfigurationError> {
        let mut configured = Self::default();
        for profile in profiles {
            if configured
                .profiles
                .insert(profile.name.clone(), profile)
                .is_some()
            {
                return Err(ConfigurationError::DuplicateProfile);
            }
        }
        if configured.profiles.is_empty() {
            return Err(ConfigurationError::MissingProfile);
        }
        for profile in configured.profiles.values() {
            if profile.production && profile.output_dimension != PRODUCTION_OUTPUT_DIMENSION {
                return Err(ConfigurationError::InvalidProfile(format!(
                    "production profile {} must output exactly {} dimensions",
                    profile.name, PRODUCTION_OUTPUT_DIMENSION
                )));
            }
            profile.identity().map_err(|e| {
                ConfigurationError::InvalidProfile(format!("{}: {e:?}", profile.name))
            })?;
        }
        Ok(configured)
    }

    pub fn profile(&self, name: &str) -> Result<&EmbeddingProfile, ConfigurationError> {
        self.profiles
            .get(name)
            .ok_or(ConfigurationError::MissingProfile)
    }

    pub fn validate_runtime_credentials(&self, name: &str) -> Result<(), ConfigurationError> {
        let profile = self.profile(name)?;
        if profile.production {
            let Some(environment_name) = profile.credential_env.as_deref() else {
                return Err(ConfigurationError::MissingCredential(profile.name.clone()));
            };
            if std::env::var(environment_name)
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(ConfigurationError::MissingCredential(
                    environment_name.to_owned(),
                ));
            }
        }
        Ok(())
    }

    pub fn production_candidates() -> Result<Self, ConfigurationError> {
        Self::new([
            EmbeddingProfile {
                name: "voyage-3-lite".into(),
                provider: "voyage-compatible".into(),
                model: "voyage-3-lite".into(),
                model_version: "pinned-v1".into(),
                native_dimension: 512,
                output_dimension: 1024,
                projection_version: "identity-v1".into(),
                configuration_fingerprint: "unset-until-configured".into(),
                production: true,
                credential_env: Some("VOYAGE_API_KEY".into()),
            },
            EmbeddingProfile {
                name: "openai-large".into(),
                provider: "openai-compatible".into(),
                model: "text-embedding-3-large".into(),
                model_version: "pinned-v1".into(),
                native_dimension: 3072,
                output_dimension: 1024,
                projection_version: "dense-projection-v1".into(),
                configuration_fingerprint: "unset-until-configured".into(),
                production: true,
                credential_env: Some("OPENAI_API_KEY".into()),
            },
            EmbeddingProfile {
                name: "cohere-v4".into(),
                provider: "cohere-compatible".into(),
                model: "embed-v4.0".into(),
                model_version: "pinned-v1".into(),
                native_dimension: 1536,
                output_dimension: 1024,
                projection_version: "dense-projection-v1".into(),
                configuration_fingerprint: "unset-until-configured".into(),
                production: true,
                credential_env: Some("COHERE_API_KEY".into()),
            },
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderError {
    Timeout { retry_after_ms: Option<u64> },
    RateLimit { retry_after_ms: Option<u64> },
    Authentication,
    InvalidRequest(String),
    Quota,
    Unavailable,
    DimensionMismatch { expected: usize, actual: usize },
    ProjectionFailure(String),
    PermanentConfiguration(String),
}

impl ProviderError {
    pub fn transient(&self) -> bool {
        matches!(
            self,
            Self::Timeout { .. } | Self::RateLimit { .. } | Self::Unavailable
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub max_concurrent: usize,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            base_delay_ms: 100,
            max_delay_ms: 5_000,
            max_concurrent: 8,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetryDirective {
    Retry { delay_ms: u64, attempt: u32 },
    Permanent,
}

pub fn retry_directive(
    error: &ProviderError,
    attempt: u32,
    policy: RetryPolicy,
    jitter_seed: u64,
) -> RetryDirective {
    if !error.transient() || attempt + 1 >= policy.max_attempts {
        return RetryDirective::Permanent;
    }
    let exponent = attempt.min(20);
    let exponential = policy
        .base_delay_ms
        .saturating_mul(1_u64 << exponent)
        .min(policy.max_delay_ms);
    let hinted = match error {
        ProviderError::Timeout { retry_after_ms } | ProviderError::RateLimit { retry_after_ms } => {
            retry_after_ms.unwrap_or(exponential)
        }
        ProviderError::Unavailable => exponential,
        _ => exponential,
    };
    // Deterministic ±25% jitter keeps tests reproducible while avoiding
    // synchronized retries in production callers that provide a random seed.
    let noise = (jitter_seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1)
        % 51) as i64
        - 25;
    let adjusted = ((hinted as i128) * (100 + noise as i128) / 100).max(1) as u64;
    RetryDirective::Retry {
        delay_ms: adjusted.min(policy.max_delay_ms),
        attempt: attempt + 1,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreaker {
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub failure_threshold: u32,
    pub cooldown_ms: u64,
    pub opened_at_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryEmbeddingCache {
    capacity: usize,
    entries: BTreeMap<String, EmbeddingVector>,
}

impl QueryEmbeddingCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: BTreeMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<EmbeddingVector> {
        self.entries.get(key).cloned()
    }

    pub fn insert(&mut self, key: impl Into<String>, vector: EmbeddingVector) {
        let key = key.into();
        self.entries.insert(key, vector);
        while self.entries.len() > self.capacity {
            if let Some(oldest) = self.entries.keys().next().cloned() {
                self.entries.remove(&oldest);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Builds a `QueryEmbeddingCache` key that fully identifies what was
/// embedded and how: provider, model, model version, input type (e.g.
/// `"query"` — Light Search only ever embeds the query text, never a
/// document, but the input type is still part of the key so this cache
/// could not silently conflate the two if it were ever reused for
/// document-side embedding), projection version, and configuration
/// fingerprint, so two different provider/profile identities never
/// collide. The query text itself is normalized (whitespace-collapsed,
/// lowercased) so two requests differing only in casing or incidental
/// whitespace share one cache entry — and so the cache never stores the
/// literal, unnormalized user input as its key material.
pub fn query_embedding_cache_key(
    identity: &ProjectionIdentity,
    input_type: &str,
    query: &str,
) -> String {
    let normalized_query = query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    format!(
        "{}:{}:{}:{}:{}:{}:{}",
        identity.provider,
        identity.model,
        identity.model_version,
        input_type,
        identity.projection_version,
        identity.configuration_fingerprint,
        normalized_query
    )
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, cooldown_ms: u64) -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            failure_threshold: failure_threshold.max(1),
            cooldown_ms,
            opened_at_ms: None,
        }
    }

    pub fn allow_request(&mut self, now_ms: u64) -> bool {
        if self.state == CircuitState::Open {
            if now_ms.saturating_sub(self.opened_at_ms.unwrap_or(now_ms)) < self.cooldown_ms {
                return false;
            }
            self.state = CircuitState::HalfOpen;
        }
        true
    }

    pub fn record_success(&mut self) {
        self.state = CircuitState::Closed;
        self.consecutive_failures = 0;
        self.opened_at_ms = None;
    }

    pub fn record_failure(&mut self, now_ms: u64) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= self.failure_threshold {
            self.state = CircuitState::Open;
            self.opened_at_ms = Some(now_ms);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionAdapter {
    pub native_dimension: usize,
    pub output_dimension: usize,
    pub version: String,
}

impl ProjectionAdapter {
    pub fn project(&self, values: &[f32]) -> Result<Vec<f32>, ProviderError> {
        if self.native_dimension == 0
            || self.output_dimension == 0
            || self.version.trim().is_empty()
        {
            return Err(ProviderError::ProjectionFailure(
                "invalid versioned projection configuration".into(),
            ));
        }
        if values.len() != self.native_dimension {
            return Err(ProviderError::DimensionMismatch {
                expected: self.native_dimension,
                actual: values.len(),
            });
        }
        if self.native_dimension == self.output_dimension {
            return Ok(values.to_vec());
        }
        // Explicit deterministic signed mixing: every native coordinate contributes
        // to an output bucket; this is never truncation or zero-padding.
        let mut output = vec![0.0; self.output_dimension];
        for (index, value) in values.iter().enumerate() {
            let bucket =
                (index.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)) % self.output_dimension;
            let sign = if (index / self.output_dimension).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            output[bucket] += value * sign;
        }
        if output.iter().all(|value| *value == 0.0) {
            return Err(ProviderError::ProjectionFailure(
                "projection produced a zero vector".into(),
            ));
        }
        Ok(output)
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

    /// Validate the complete projection identity before a dense result can be
    /// fused. Callers must fail the dense channel rather than mixing profiles.
    pub fn validate_identity(&self, identity: &ProjectionIdentity) -> Result<(), RetrievalError> {
        if self
            .vectors
            .values()
            .any(|(found, vector)| found != identity || vector.values.len() != identity.dimension)
        {
            return Err(RetrievalError::Provider(ProviderError::DimensionMismatch {
                expected: identity.dimension,
                actual: self
                    .vectors
                    .values()
                    .find_map(|(_, vector)| {
                        (vector.values.len() != identity.dimension).then_some(vector.values.len())
                    })
                    .unwrap_or(identity.dimension),
            }));
        }
        Ok(())
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

/// Deterministic aggressive-mode reranking. Unlike Light Search's fusion
/// ordering, this gives a small, explainable boost to claims covering more of
/// the normalized task terms while retaining the original lexical score as
/// the primary signal.
pub fn rerank_aggressive_hits(hits: &mut [LexicalHit], query: &str) {
    let terms: BTreeSet<String> = tokenize(query).into_iter().collect();
    hits.sort_by(|left, right| {
        aggressive_rerank_score(right, &terms)
            .total_cmp(&aggressive_rerank_score(left, &terms))
            .then_with(|| left.record.memory_id.cmp(&right.record.memory_id))
    });
}

fn aggressive_rerank_score(hit: &LexicalHit, terms: &BTreeSet<String>) -> f32 {
    let covered = tokenize(&hit.record.claim)
        .into_iter()
        .filter(|term| terms.contains(term))
        .count();
    hit.score + covered as f32 * 0.01
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
    Provider(ProviderError),
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

/// Explicit bounds on Cross-Map Light Search expansion. `expand_cross_map_candidates`
/// is opt-in — nothing calls it automatically, and ordinary `light_search`
/// output is unaffected by its existence — so a caller who never invokes it
/// gets exactly Phase E's existing behavior (non-negotiable decision #9:
/// Phase F must not silently alter Phase E ranking semantics).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrossMapExpansionBudget {
    pub max_mappings: usize,
    pub max_candidates_per_mapping: usize,
    pub max_total_candidates: usize,
}

impl Default for CrossMapExpansionBudget {
    /// Small and conservative by design: Cross-Map expansion is meant to
    /// surface a few explainable cross-Area hints, not to become a second
    /// aggressive search hiding inside Light Search. A future benchmark may
    /// move these numbers; they are not permanent truth.
    fn default() -> Self {
        Self {
            max_mappings: 3,
            max_candidates_per_mapping: 5,
            max_total_candidates: 10,
        }
    }
}

/// One approved Cross-Map mapping being offered for expansion, together with
/// records the caller has *already* authorized as visible in
/// `mapping.target_area_id` for this actor/purpose. This function does not
/// perform that authorization itself — exactly like `LexicalIndex::search`
/// and dense retrieval, the server derives permitted candidates before this
/// function ever runs; it only decides which already-authorized records may
/// cross an approved mapping, and how many.
#[derive(Clone, Debug, PartialEq)]
pub struct CrossMapExpansionSource {
    pub mapping: CrossMapMapping,
    /// Expiry set at approval time. `CrossMapMapping` itself carries no
    /// expiry field (see `cross_map.rs`'s expiry-model doc comment); the
    /// caller must supply whatever `cross_map::CrossMapStore` recorded.
    pub expires_at: Option<OffsetDateTime>,
    pub candidates: Vec<MemoryRecord>,
}

/// Cross-Map Light Search expansion. A blocked, expired, revoked, rejected,
/// superseded, or semantically-`Blocked` mapping contributes zero
/// candidates — re-checked here via `cross_map::is_traversable` even though
/// the caller is expected to have filtered already, because mapping
/// permission must be verified immediately before candidate generation,
/// never trusted from an earlier check alone (non-negotiable decision #5).
/// Expansion only follows a mapping's declared direction: a mapping is only
/// used when `active_area_id == mapping.source_area_id`, never the reverse,
/// keeping Cross-Map conservative and explicit rather than bidirectional by
/// accident. Every returned `RetrievalResult` keeps the record's *original*
/// `area_id` (the source Area it actually lives in, not the active Area)
/// and records the mapping path in `warnings`, so meaning is never
/// flattened across Areas (non-negotiable decision #6 / roadmap: "Cross-Map
/// results preserve original labels and mapping paths").
pub fn expand_cross_map_candidates(
    active_area_id: AreaId,
    now: OffsetDateTime,
    sources: &[CrossMapExpansionSource],
    budget: CrossMapExpansionBudget,
) -> (Vec<RetrievalResult>, bool) {
    let mut results = Vec::new();
    let mut truncated = false;
    let mut mappings_used = 0usize;

    for source in sources {
        if mappings_used >= budget.max_mappings {
            truncated = true;
            break;
        }
        if source.mapping.source_area_id != active_area_id {
            // Conservative by design: only the mapping's declared direction
            // is traversable, never the reverse.
            continue;
        }
        if source.mapping.relation == CrossMapRelation::Blocked {
            continue;
        }
        if !cross_map::is_traversable(&source.mapping, now, source.expires_at) {
            continue;
        }
        mappings_used += 1;

        let mapping_path = format!(
            "cross_map:{}:{:?}:{}->{}",
            source.mapping.cross_map_mapping_id.as_uuid(),
            source.mapping.relation,
            source.mapping.source_area_id.as_uuid(),
            source.mapping.target_area_id.as_uuid()
        );

        for (taken_from_this_mapping, record) in source.candidates.iter().enumerate() {
            if taken_from_this_mapping >= budget.max_candidates_per_mapping
                || results.len() >= budget.max_total_candidates
            {
                truncated = true;
                break;
            }
            results.push(RetrievalResult {
                memory_id: record.memory_id,
                area_id: record.area_id,
                claim: record.claim.clone(),
                reason: record.reason.clone(),
                provenance: record.evidence.clone(),
                applicability: record.applies_when.clone(),
                warnings: vec![mapping_path.clone()],
                estimated_tokens: estimate_tokens(record),
            });
        }
        if results.len() >= budget.max_total_candidates {
            break;
        }
    }

    (results, truncated)
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
            owner_actor_id: None,
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
                actor_id: None,
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
    fn aggressive_rerank_is_deterministic_and_task_term_sensitive() {
        let (tenant, area, _, first, second, _) = ids();
        let mut hits = vec![
            LexicalHit {
                record: record(tenant, area, first, "renewal"),
                score: 1.0,
                rank: 1,
                reason: "fixture".into(),
            },
            LexicalHit {
                record: record(tenant, area, second, "renewal security review"),
                score: 0.995,
                rank: 2,
                reason: "fixture".into(),
            },
        ];
        rerank_aggressive_hits(&mut hits, "renewal security review");
        assert_eq!(hits[0].record.memory_id, second);
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

    #[test]
    fn two_production_profiles_are_1024_dimensional_and_secret_free() {
        let config = EmbeddingConfiguration::production_candidates().unwrap();
        for name in ["openai-large", "cohere-v4"] {
            let profile = config.profile(name).unwrap();
            assert_eq!(profile.output_dimension, PRODUCTION_OUTPUT_DIMENSION);
            assert!(profile
                .credential_env
                .as_deref()
                .unwrap()
                .ends_with("_API_KEY"));
            assert!(!format!("{profile:?}").contains("sk-"));
            assert_eq!(profile.identity().unwrap().dimension, 1024);
        }
    }

    #[test]
    fn projection_is_explicit_and_never_truncates_or_pads() {
        let adapter = ProjectionAdapter {
            native_dimension: 3,
            output_dimension: 1024,
            version: "projection-v1".into(),
        };
        let projected = adapter.project(&[1.0, 2.0, 3.0]).unwrap();
        assert_eq!(projected.len(), 1024);
        assert!(projected.iter().filter(|value| **value != 0.0).count() >= 3);
        assert!(matches!(
            adapter.project(&[1.0]),
            Err(ProviderError::DimensionMismatch { .. })
        ));
        assert!(matches!(
            (ProjectionAdapter {
                native_dimension: 3,
                output_dimension: 3,
                version: String::new()
            })
            .project(&[1.0, 2.0, 3.0]),
            Err(ProviderError::ProjectionFailure(_))
        ));
    }

    #[test]
    fn provider_errors_classify_retryable_failures() {
        assert!(ProviderError::Timeout {
            retry_after_ms: Some(50)
        }
        .transient());
        assert!(ProviderError::RateLimit {
            retry_after_ms: None
        }
        .transient());
        assert!(ProviderError::Unavailable.transient());
        assert!(!ProviderError::Authentication.transient());
        assert!(!ProviderError::Quota.transient());
        assert!(!ProviderError::PermanentConfiguration("missing endpoint".into()).transient());
        for error in [
            ProviderError::Authentication,
            ProviderError::InvalidRequest("bad input".into()),
            ProviderError::Quota,
            ProviderError::DimensionMismatch {
                expected: 1024,
                actual: 512,
            },
            ProviderError::ProjectionFailure("invalid adapter".into()),
        ] {
            assert!(!error.transient());
        }
    }

    #[test]
    fn production_configuration_rejects_non_1024_output() {
        let result = EmbeddingConfiguration::new([EmbeddingProfile {
            name: "bad-production".into(),
            provider: "test".into(),
            model: "bad".into(),
            model_version: "1".into(),
            native_dimension: 384,
            output_dimension: 384,
            projection_version: "v1".into(),
            configuration_fingerprint: "fp".into(),
            production: true,
            credential_env: None,
        }]);
        assert!(matches!(result, Err(ConfigurationError::InvalidProfile(_))));
    }

    #[test]
    fn retry_policy_honors_hints_caps_and_permanent_errors() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 500,
            max_concurrent: 2,
        };
        let retry = retry_directive(
            &ProviderError::RateLimit {
                retry_after_ms: Some(1_000),
            },
            0,
            policy,
            7,
        );
        assert!(matches!(
            retry,
            RetryDirective::Retry {
                delay_ms: 500,
                attempt: 1
            }
        ));
        assert_eq!(
            retry_directive(&ProviderError::Authentication, 0, policy, 7),
            RetryDirective::Permanent
        );
        assert_eq!(
            retry_directive(&ProviderError::Unavailable, 2, policy, 7),
            RetryDirective::Permanent
        );
    }

    #[test]
    fn circuit_breaker_opens_and_recovers_after_cooldown() {
        let mut breaker = CircuitBreaker::new(2, 100);
        assert!(breaker.allow_request(0));
        breaker.record_failure(10);
        assert!(breaker.allow_request(20));
        breaker.record_failure(30);
        assert_eq!(breaker.state, CircuitState::Open);
        assert!(!breaker.allow_request(100));
        assert!(breaker.allow_request(131));
        assert_eq!(breaker.state, CircuitState::HalfOpen);
        breaker.record_success();
        assert_eq!(breaker.state, CircuitState::Closed);
    }

    #[test]
    fn query_embedding_cache_is_bounded_and_profile_scoped_by_key() {
        let identity = ProjectionIdentity::new("test", "model", "1", 2, "v1", "fp").unwrap();
        let vector = EmbeddingVector::normalized(vec![1.0, 0.0], identity.dimension).unwrap();
        let mut cache = QueryEmbeddingCache::new(2);
        cache.insert("provider:model:version:query-a", vector.clone());
        cache.insert("provider:model:version:query-b", vector.clone());
        assert!(cache.get("provider:model:version:query-a").is_some());
        cache.insert("other-provider:model:version:query-c", vector);
        assert_eq!(cache.len(), 2);
        assert!(cache.get("provider:model:version:query-b").is_some());
    }

    #[test]
    fn query_embedding_cache_key_normalizes_and_isolates_by_full_identity() {
        let identity =
            ProjectionIdentity::new("voyage-compatible", "voyage-3-lite", "1", 2, "v1", "fp-a")
                .unwrap();

        // Same identity, same query (modulo casing/whitespace) -> same key,
        // so a real cache hit occurs on the second lookup.
        let key_a = query_embedding_cache_key(&identity, "query", "Renewal Security");
        let key_a_normalized =
            query_embedding_cache_key(&identity, "query", "  renewal   security  ");
        assert_eq!(key_a, key_a_normalized);

        let mut cache = QueryEmbeddingCache::new(4);
        let vector = EmbeddingVector::normalized(vec![1.0, 0.0], identity.dimension).unwrap();
        cache.insert(key_a.clone(), vector.clone());
        assert!(
            cache.get(&key_a_normalized).is_some(),
            "normalized query must hit the same cache entry"
        );

        // A different query text is a genuine cache miss.
        let different_query_key = query_embedding_cache_key(&identity, "query", "different query");
        assert!(cache.get(&different_query_key).is_none());

        // Profile isolation: identical query text, different provider ->
        // different key, never collides.
        let other_provider =
            ProjectionIdentity::new("openai-compatible", "voyage-3-lite", "1", 2, "v1", "fp-a")
                .unwrap();
        assert_ne!(
            key_a,
            query_embedding_cache_key(&other_provider, "query", "Renewal Security")
        );

        // Profile isolation: identical provider/model, different projection
        // version -> different key (a re-projection must never reuse a
        // stale vector cached under the old projection).
        let other_projection =
            ProjectionIdentity::new("voyage-compatible", "voyage-3-lite", "1", 2, "v2", "fp-a")
                .unwrap();
        assert_ne!(
            key_a,
            query_embedding_cache_key(&other_projection, "query", "Renewal Security")
        );

        // Input type is part of the key even though Light Search only ever
        // uses "query" today.
        assert_ne!(
            key_a,
            query_embedding_cache_key(&identity, "document", "Renewal Security")
        );
    }

    fn mapping(
        tenant_id: TenantId,
        source_area_id: AreaId,
        target_area_id: AreaId,
        relation: CrossMapRelation,
        state: engrave_contracts::CrossMapMappingState,
    ) -> CrossMapMapping {
        CrossMapMapping {
            cross_map_mapping_id: engrave_contracts::CrossMapMappingId::new_v7(),
            tenant_id,
            source_area_id,
            target_area_id,
            source_map_version_id: engrave_contracts::MapVersionId::new_v7(),
            target_map_version_id: engrave_contracts::MapVersionId::new_v7(),
            relation,
            state,
            rationale: "shared account concept".into(),
            version: 1,
        }
    }

    #[test]
    fn cross_map_expansion_preserves_original_area_and_mapping_path() {
        use engrave_contracts::CrossMapMappingState;
        let tenant_id = TenantId::new_v7();
        let active_area = AreaId::new_v7();
        let other_area = AreaId::new_v7();
        let now = OffsetDateTime::UNIX_EPOCH;

        let approved = mapping(
            tenant_id,
            active_area,
            other_area,
            CrossMapRelation::RelatedTo,
            CrossMapMappingState::Approved,
        );
        let candidate = record(
            tenant_id,
            other_area,
            MemoryId::new_v7(),
            "Marketing account view shares the Sales account concept",
        );

        let (results, truncated) = expand_cross_map_candidates(
            active_area,
            now,
            &[CrossMapExpansionSource {
                mapping: approved.clone(),
                expires_at: None,
                candidates: vec![candidate.clone()],
            }],
            CrossMapExpansionBudget::default(),
        );

        assert!(!truncated);
        assert_eq!(results.len(), 1);
        // Original Area label is preserved — never relabeled to the active Area.
        assert_eq!(results[0].area_id, other_area);
        assert_eq!(results[0].memory_id, candidate.memory_id);
        assert!(
            results[0].warnings[0].contains(&approved.cross_map_mapping_id.as_uuid().to_string())
        );
        assert!(results[0].warnings[0].contains(&active_area.as_uuid().to_string()));
        assert!(results[0].warnings[0].contains(&other_area.as_uuid().to_string()));
    }

    #[test]
    fn cross_map_expansion_yields_zero_candidates_for_every_non_approved_state() {
        use engrave_contracts::CrossMapMappingState;
        let tenant_id = TenantId::new_v7();
        let active_area = AreaId::new_v7();
        let other_area = AreaId::new_v7();
        let now = OffsetDateTime::UNIX_EPOCH;
        let candidate = record(
            tenant_id,
            other_area,
            MemoryId::new_v7(),
            "should never surface",
        );

        for state in [
            CrossMapMappingState::Proposed,
            CrossMapMappingState::Rejected,
            CrossMapMappingState::Blocked,
            CrossMapMappingState::Expired,
            CrossMapMappingState::Revoked,
            CrossMapMappingState::Superseded,
        ] {
            let unusable = mapping(
                tenant_id,
                active_area,
                other_area,
                CrossMapRelation::RelatedTo,
                state,
            );
            let (results, _) = expand_cross_map_candidates(
                active_area,
                now,
                &[CrossMapExpansionSource {
                    mapping: unusable,
                    expires_at: None,
                    candidates: vec![candidate.clone()],
                }],
                CrossMapExpansionBudget::default(),
            );
            assert!(
                results.is_empty(),
                "state {state:?} must yield zero candidates"
            );
        }
    }

    #[test]
    fn cross_map_expansion_ignores_expired_and_wrong_direction_and_blocked_relation() {
        use engrave_contracts::CrossMapMappingState;
        let tenant_id = TenantId::new_v7();
        let active_area = AreaId::new_v7();
        let other_area = AreaId::new_v7();
        let now = OffsetDateTime::UNIX_EPOCH;
        let candidate = record(tenant_id, other_area, MemoryId::new_v7(), "never surfaces");

        // Approved but already expired.
        let expired = mapping(
            tenant_id,
            active_area,
            other_area,
            CrossMapRelation::RelatedTo,
            CrossMapMappingState::Approved,
        );
        let (results, _) = expand_cross_map_candidates(
            active_area,
            now,
            &[CrossMapExpansionSource {
                mapping: expired,
                expires_at: Some(now - time::Duration::seconds(1)),
                candidates: vec![candidate.clone()],
            }],
            CrossMapExpansionBudget::default(),
        );
        assert!(results.is_empty());

        // Approved, unexpired, but the active Area is the mapping's TARGET,
        // not its declared source — must not be traversed in reverse.
        let wrong_direction = mapping(
            tenant_id,
            other_area,
            active_area,
            CrossMapRelation::RelatedTo,
            CrossMapMappingState::Approved,
        );
        let (results, _) = expand_cross_map_candidates(
            active_area,
            now,
            &[CrossMapExpansionSource {
                mapping: wrong_direction,
                expires_at: None,
                candidates: vec![candidate.clone()],
            }],
            CrossMapExpansionBudget::default(),
        );
        assert!(results.is_empty());

        // Approved, correct direction, but the semantic relation itself is
        // Blocked — defense in depth even if lifecycle state says Approved.
        let blocked_relation = mapping(
            tenant_id,
            active_area,
            other_area,
            CrossMapRelation::Blocked,
            CrossMapMappingState::Approved,
        );
        let (results, _) = expand_cross_map_candidates(
            active_area,
            now,
            &[CrossMapExpansionSource {
                mapping: blocked_relation,
                expires_at: None,
                candidates: vec![candidate],
            }],
            CrossMapExpansionBudget::default(),
        );
        assert!(results.is_empty());
    }

    #[test]
    fn cross_map_expansion_is_bounded_by_budget() {
        use engrave_contracts::CrossMapMappingState;
        let tenant_id = TenantId::new_v7();
        let active_area = AreaId::new_v7();
        let other_area = AreaId::new_v7();
        let now = OffsetDateTime::UNIX_EPOCH;
        let approved = mapping(
            tenant_id,
            active_area,
            other_area,
            CrossMapRelation::RelatedTo,
            CrossMapMappingState::Approved,
        );
        let candidates: Vec<_> = (0..10)
            .map(|i| {
                record(
                    tenant_id,
                    other_area,
                    MemoryId::new_v7(),
                    &format!("hit {i}"),
                )
            })
            .collect();

        let (results, truncated) = expand_cross_map_candidates(
            active_area,
            now,
            &[CrossMapExpansionSource {
                mapping: approved,
                expires_at: None,
                candidates,
            }],
            CrossMapExpansionBudget {
                max_mappings: 1,
                max_candidates_per_mapping: 2,
                max_total_candidates: 2,
            },
        );
        assert_eq!(results.len(), 2);
        assert!(truncated);
    }
}
