//! SQLx/PostgreSQL and S3-compatible object-store adapters.
//!
//! The adapters keep framework concerns out of the storage boundary. SQLx
//! queries are runtime queries until the committed offline cache is generated
//! against the canonical migration; the migration itself remains the source
//! of truth for schema shape.
#![forbid(unsafe_code)]

pub mod notion;
pub use notion::NotionConnector;

use arrow_array::{
    types::Float32Type, Array, FixedSizeListArray, RecordBatch, RecordBatchIterator,
    RecordBatchReader, StringArray, UInt32Array,
};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use aws_sdk_s3::{primitives::ByteStream, Client as S3Client};
use engrave_contracts::{
    AreaId, Entity, EntityId, EntityState, MapVersionId, MemoryId, OperationId, OperationState,
    Origin, Relationship, RelationshipId, RelationshipState, TenantId,
};
use engrave_core::{
    retry_directive, ActorRole, AggressiveIntent, ApplicationError, AuthorizationContext, Citation,
    DenseHit, DeterministicEmbeddingProvider, DomainEvent, EmbeddingProvider, IdempotencyKey,
    LexicalHit, MemoryRecord, ObjectStore, ProjectionAdapter, ProjectionIdentity, ProviderError,
    Repository, RetryDirective, RetryPolicy, Rule, RuleDecision, SearchBudgets, SearchRequest,
    SearchTrace, VersionToken, Visibility,
};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

async fn retry_provider_call<T, F, Fut>(mut call: F) -> Result<T, ProviderError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    let policy = RetryPolicy::default();
    let mut attempt = 0;
    loop {
        match call().await {
            Ok(value) => return Ok(value),
            Err(error) => match retry_directive(&error, attempt, policy, attempt as u64 + 17) {
                RetryDirective::Retry {
                    delay_ms,
                    attempt: next,
                } => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    attempt = next;
                }
                RetryDirective::Permanent => return Err(error),
            },
        }
    }
}

#[derive(Clone)]
pub struct VoyageEmbeddingClient {
    http: reqwest::Client,
    endpoint: String,
    model: String,
    api_key: String,
    native_dimension: usize,
}

#[derive(Clone)]
pub struct OpenAiEmbeddingClient {
    http: reqwest::Client,
    endpoint: String,
    model: String,
    api_key: String,
    native_dimension: usize,
}

impl OpenAiEmbeddingClient {
    pub fn from_env() -> Result<Self, ProviderError> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            ProviderError::PermanentConfiguration("OPENAI_API_KEY is missing".into())
        })?;
        if api_key.trim().is_empty() {
            return Err(ProviderError::PermanentConfiguration(
                "OPENAI_API_KEY is empty".into(),
            ));
        }
        let endpoint = std::env::var("OPENAI_EMBEDDING_ENDPOINT")
            .unwrap_or_else(|_| "https://api.openai.com/v1/embeddings".into());
        let model = std::env::var("OPENAI_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "text-embedding-3-large".into());
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|_| {
                ProviderError::PermanentConfiguration("invalid HTTP client configuration".into())
            })?;
        Ok(Self {
            http,
            endpoint,
            model,
            api_key,
            native_dimension: 3072,
        })
    }

    async fn request_embeddings_once(
        &self,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({"input": inputs, "model": self.model}))
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ProviderError::Timeout {
                        retry_after_ms: None,
                    }
                } else {
                    ProviderError::Unavailable
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                401 | 403 => ProviderError::Authentication,
                402 => ProviderError::Quota,
                408 => ProviderError::Timeout {
                    retry_after_ms: None,
                },
                429 => ProviderError::RateLimit {
                    retry_after_ms: response
                        .headers()
                        .get("retry-after")
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        .map(|seconds| seconds * 1000),
                },
                400..=499 => ProviderError::InvalidRequest(format!(
                    "provider returned HTTP {}",
                    status.as_u16()
                )),
                500..=599 => ProviderError::Unavailable,
                _ => ProviderError::Unavailable,
            });
        }
        let body: VoyageResponse = response
            .json()
            .await
            .map_err(|_| ProviderError::InvalidRequest("invalid embedding response".into()))?;
        validate_embedding_batch(body.data, inputs.len(), self.native_dimension)
    }

    async fn request_embeddings(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        retry_provider_call(|| self.request_embeddings_once(inputs)).await
    }

    async fn embed_once(&self, input: &str) -> Result<Vec<f32>, ProviderError> {
        self.request_embeddings(&[input.to_owned()])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::InvalidRequest("provider returned no embedding".into()))
    }

    pub async fn embed(&self, input: &str) -> Result<Vec<f32>, ProviderError> {
        self.embed_once(input).await
    }

    pub async fn embed_batch(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        self.request_embeddings(inputs).await
    }

    pub async fn embed_projected(
        &self,
        input: &str,
        identity: &ProjectionIdentity,
    ) -> Result<Vec<f32>, ProviderError> {
        let native = self.embed(input).await?;
        ProjectionAdapter {
            native_dimension: self.native_dimension,
            output_dimension: identity.dimension,
            version: identity.projection_version.clone(),
        }
        .project(&native)
    }
}

impl VoyageEmbeddingClient {
    pub fn from_env() -> Result<Self, ProviderError> {
        let api_key = std::env::var("VOYAGE_API_KEY").map_err(|_| {
            ProviderError::PermanentConfiguration("VOYAGE_API_KEY is missing".into())
        })?;
        if api_key.trim().is_empty() {
            return Err(ProviderError::PermanentConfiguration(
                "VOYAGE_API_KEY is empty".into(),
            ));
        }
        let endpoint = std::env::var("VOYAGE_API_ENDPOINT")
            .unwrap_or_else(|_| "https://api.voyageai.com/v1/embeddings".into());
        let model =
            std::env::var("VOYAGE_EMBEDDING_MODEL").unwrap_or_else(|_| "voyage-3-lite".into());
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|_| {
                ProviderError::PermanentConfiguration("invalid HTTP client configuration".into())
            })?;
        Ok(Self {
            http,
            endpoint,
            model,
            api_key,
            native_dimension: 512,
        })
    }

    async fn embed_once(&self, input: &str) -> Result<Vec<f32>, ProviderError> {
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({"input": [input], "model": self.model}))
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ProviderError::Timeout {
                        retry_after_ms: None,
                    }
                } else {
                    ProviderError::Unavailable
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            let retry_after_ms = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(|seconds| seconds * 1000);
            return Err(match status.as_u16() {
                401 | 403 => ProviderError::Authentication,
                402 => ProviderError::Quota,
                408 => ProviderError::Timeout { retry_after_ms },
                429 => ProviderError::RateLimit { retry_after_ms },
                400..=499 => ProviderError::InvalidRequest(format!(
                    "provider returned HTTP {}",
                    status.as_u16()
                )),
                500..=599 => ProviderError::Unavailable,
                _ => ProviderError::Unavailable,
            });
        }
        let body: VoyageResponse = response
            .json()
            .await
            .map_err(|_| ProviderError::InvalidRequest("invalid embedding response".into()))?;
        let values = body
            .data
            .into_iter()
            .next()
            .map(|item| item.embedding)
            .ok_or_else(|| {
                ProviderError::InvalidRequest("provider returned no embedding".into())
            })?;
        if values.len() != self.native_dimension {
            return Err(ProviderError::DimensionMismatch {
                expected: self.native_dimension,
                actual: values.len(),
            });
        }
        Ok(values)
    }

    pub async fn embed(&self, input: &str) -> Result<Vec<f32>, ProviderError> {
        retry_provider_call(|| self.embed_once(input)).await
    }

    pub async fn embed_projected(
        &self,
        input: &str,
        identity: &ProjectionIdentity,
    ) -> Result<Vec<f32>, ProviderError> {
        let native = self.embed(input).await?;
        ProjectionAdapter {
            native_dimension: self.native_dimension,
            output_dimension: identity.dimension,
            version: identity.projection_version.clone(),
        }
        .project(&native)
    }

    /// Batch endpoint used by durable re-embedding/index jobs. The returned
    /// order is the provider response order and is checked against input size.
    async fn embed_batch_once(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({"input": inputs, "model": self.model}))
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ProviderError::Timeout {
                        retry_after_ms: None,
                    }
                } else {
                    ProviderError::Unavailable
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                401 | 403 => ProviderError::Authentication,
                402 => ProviderError::Quota,
                408 => ProviderError::Timeout {
                    retry_after_ms: None,
                },
                429 => ProviderError::RateLimit {
                    retry_after_ms: response
                        .headers()
                        .get("retry-after")
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        .map(|seconds| seconds * 1000),
                },
                400..=499 => ProviderError::InvalidRequest(format!(
                    "provider returned HTTP {}",
                    status.as_u16()
                )),
                500..=599 => ProviderError::Unavailable,
                _ => ProviderError::Unavailable,
            });
        }
        let body: VoyageResponse = response
            .json()
            .await
            .map_err(|_| ProviderError::InvalidRequest("invalid embedding response".into()))?;
        validate_embedding_batch(body.data, inputs.len(), self.native_dimension)
    }

    pub async fn embed_batch(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        retry_provider_call(|| self.embed_batch_once(inputs)).await
    }
}

#[derive(serde::Deserialize)]
struct VoyageResponse {
    data: Vec<VoyageEmbedding>,
}

#[derive(serde::Deserialize)]
struct VoyageEmbedding {
    embedding: Vec<f32>,
}

fn validate_embedding_batch(
    data: Vec<VoyageEmbedding>,
    expected_items: usize,
    expected_dimension: usize,
) -> Result<Vec<Vec<f32>>, ProviderError> {
    if data.len() != expected_items {
        return Err(ProviderError::InvalidRequest(
            "provider returned a partial batch".into(),
        ));
    }
    data.into_iter()
        .map(|item| {
            if item.embedding.len() != expected_dimension {
                return Err(ProviderError::DimensionMismatch {
                    expected: expected_dimension,
                    actual: item.embedding.len(),
                });
            }
            Ok(item.embedding)
        })
        .collect()
}

/// A PostgreSQL adapter with a small, explicit repository surface.
#[derive(Clone, Debug)]
pub struct PgRepository {
    pool: PgPool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DurableLease {
    pub operation_id: OperationId,
    pub lease_token: String,
    pub attempt: u32,
    pub checkpoint: Option<serde_json::Value>,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceEvidence {
    pub citation: Citation,
    pub content: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthorizedGraphSlice {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}

/// A rebuildable LanceDB row. Canonical claim/lifecycle data stays in
/// PostgreSQL; these fields exist so authorization can be pushed into the
/// vector query before any candidate is returned.
#[derive(Clone)]
pub struct LanceProjectionRow {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub memory_id: MemoryId,
    pub owner_actor_id: Option<Uuid>,
    pub scope: String,
    pub state: String,
    pub identity: engrave_core::ProjectionIdentity,
    pub vector: engrave_core::EmbeddingVector,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LanceHit {
    pub tenant_id: TenantId,
    pub area_id: AreaId,
    pub memory_id: MemoryId,
    pub owner_actor_id: Option<Uuid>,
    pub distance: f32,
}

#[derive(Clone)]
pub struct LanceProjectionAdapter {
    db: lancedb::Connection,
    table_name: String,
}

impl LanceProjectionAdapter {
    pub async fn connect(
        path: &str,
        table_name: impl Into<String>,
    ) -> Result<Self, ApplicationError> {
        let db = lancedb::connect(path).execute().await.map_err(|_| {
            ApplicationError::DependencyUnavailable {
                dependency: "lancedb",
            }
        })?;
        Ok(Self {
            db,
            table_name: table_name.into(),
        })
    }

    /// Rebuilds the projection from canonical rows. Rebuild/overwrite makes
    /// stale and deleted vector rows disappear instead of relying on an
    /// eventually-consistent delete sequence.
    pub async fn reconcile(
        &self,
        rows: &[LanceProjectionRow],
    ) -> Result<engrave_core::ReconciliationReport, ApplicationError> {
        if rows.is_empty() {
            return Ok(engrave_core::ReconciliationReport::default());
        }
        let dimension = rows[0].identity.dimension;
        if rows
            .iter()
            .any(|row| row.identity != rows[0].identity || row.vector.values.len() != dimension)
        {
            return Err(ApplicationError::InvalidRequest {
                message: "mixed LanceDB projection dimensions".into(),
            });
        }
        let schema = std::sync::Arc::new(Schema::new(vec![
            Field::new("tenant_id", DataType::Utf8, false),
            Field::new("area_id", DataType::Utf8, false),
            Field::new("memory_id", DataType::Utf8, false),
            Field::new("owner_actor_id", DataType::Utf8, true),
            Field::new("scope", DataType::Utf8, false),
            Field::new("state", DataType::Utf8, false),
            Field::new("provider", DataType::Utf8, false),
            Field::new("model", DataType::Utf8, false),
            Field::new("model_version", DataType::Utf8, false),
            Field::new("projection_version", DataType::Utf8, false),
            Field::new("configuration_fingerprint", DataType::Utf8, false),
            Field::new("native_dimension", DataType::UInt32, false),
            Field::new("dimension", DataType::UInt32, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    std::sync::Arc::new(Field::new("item", DataType::Float32, true)),
                    dimension as i32,
                ),
                false,
            ),
        ]));
        let vectors = rows.iter().map(|row| {
            Some(
                row.vector
                    .values
                    .iter()
                    .copied()
                    .map(Some)
                    .collect::<Vec<_>>(),
            )
        });
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                std::sync::Arc::new(StringArray::from(
                    rows.iter()
                        .map(|row| row.tenant_id.as_uuid().to_string())
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter()
                        .map(|row| row.area_id.as_uuid().to_string())
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter()
                        .map(|row| row.memory_id.as_uuid().to_string())
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter()
                        .map(|row| row.owner_actor_id.map(|id| id.to_string()))
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter().map(|row| row.scope.clone()).collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter().map(|row| row.state.clone()).collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter()
                        .map(|row| row.identity.provider.clone())
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter()
                        .map(|row| row.identity.model.clone())
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter()
                        .map(|row| row.identity.model_version.clone())
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter()
                        .map(|row| row.identity.projection_version.clone())
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(StringArray::from(
                    rows.iter()
                        .map(|row| row.identity.configuration_fingerprint.clone())
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(UInt32Array::from(
                    rows.iter()
                        .map(|row| row.identity.native_dimension as u32)
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(UInt32Array::from(
                    rows.iter()
                        .map(|row| row.identity.dimension as u32)
                        .collect::<Vec<_>>(),
                )),
                std::sync::Arc::new(
                    FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                        vectors,
                        dimension as i32,
                    ),
                ),
            ],
        )
        .map_err(|_| ApplicationError::InvalidRequest {
            message: "invalid LanceDB projection batch".into(),
        })?;
        let batches: Box<dyn RecordBatchReader + Send> = Box::new(RecordBatchIterator::new(
            vec![Ok(batch)].into_iter(),
            schema,
        ));
        self.db
            .create_table(&self.table_name, batches)
            .mode(lancedb::database::CreateTableMode::Overwrite)
            .execute()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "lancedb",
            })?;
        Ok(engrave_core::ReconciliationReport {
            verified: 0,
            repaired: rows.len(),
            removed: 0,
        })
    }

    /// Builds the optional IVF-Flat ANN index after a worker reconciliation.
    /// Index creation is explicit and remains a worker-side write.
    pub async fn build_ann_index(&self) -> Result<(), ApplicationError> {
        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "lancedb",
            })?;
        table
            .create_index(
                &["vector"],
                lancedb::index::Index::IvfFlat(
                    lancedb::index::vector::IvfFlatIndexBuilder::default(),
                ),
            )
            .execute()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "lancedb",
            })?;
        Ok(())
    }

    /// Exact vector search with authorization pushed into LanceDB's scalar
    /// filter. The caller must still rehydrate and re-authorize canonical
    /// PostgreSQL records before packet construction.
    pub async fn search_exact(
        &self,
        authorization: &engrave_core::AuthorizationContext,
        query: &engrave_core::EmbeddingVector,
        identity: &engrave_core::ProjectionIdentity,
        entry_limit: usize,
    ) -> Result<Vec<LanceHit>, ApplicationError> {
        self.search_vector(authorization, query, identity, entry_limit, true)
            .await
    }

    /// ANN vector search using the configured LanceDB vector index. The same
    /// authorization filter and canonical rehydration requirements as exact
    /// search apply.
    pub async fn search_ann(
        &self,
        authorization: &engrave_core::AuthorizationContext,
        query: &engrave_core::EmbeddingVector,
        identity: &engrave_core::ProjectionIdentity,
        entry_limit: usize,
    ) -> Result<Vec<LanceHit>, ApplicationError> {
        self.search_vector(authorization, query, identity, entry_limit, false)
            .await
    }

    async fn search_vector(
        &self,
        authorization: &engrave_core::AuthorizationContext,
        query: &engrave_core::EmbeddingVector,
        identity: &engrave_core::ProjectionIdentity,
        entry_limit: usize,
        bypass_vector_index: bool,
    ) -> Result<Vec<LanceHit>, ApplicationError> {
        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "lancedb",
            })?;
        let areas = authorization
            .permitted_area_ids
            .iter()
            .map(|area| format!("'{}'", area.as_uuid()))
            .collect::<Vec<_>>()
            .join(", ");
        let actor = authorization
            .actor_id
            .map(|id| format!("'{}'", id))
            .unwrap_or_else(|| "NULL".into());
        let enterprise = matches!(authorization.role, ActorRole::EnterpriseAdmin);
        let filter = format!(
            "tenant_id = '{}' AND area_id IN ({}) AND state = 'current' AND dimension = {} AND ( ((scope LIKE 'personal%' OR scope LIKE 'private%') AND owner_actor_id = {}) OR (scope NOT LIKE 'personal%' AND scope NOT LIKE 'private%' AND (scope NOT LIKE 'enterprise%' OR {})) )",
            authorization.tenant_id.as_uuid(), areas, identity.dimension, actor, enterprise
        );
        let vector_query = table
            .vector_search(query.values.clone())
            .map_err(|_| ApplicationError::InvalidRequest {
                message: "invalid query vector".into(),
            })?
            .column("vector")
            .only_if(filter)
            .limit(entry_limit.min(30));
        let vector_query = if bypass_vector_index {
            vector_query.bypass_vector_index()
        } else {
            vector_query
        };
        let stream =
            vector_query
                .execute()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "lancedb",
                })?;
        let batches = stream.try_collect::<Vec<_>>().await.map_err(|_| {
            ApplicationError::DependencyUnavailable {
                dependency: "lancedb",
            }
        })?;
        let mut hits = Vec::new();
        for batch in batches {
            let tenant = batch
                .column_by_name("tenant_id")
                .and_then(|column| column.as_any().downcast_ref::<StringArray>());
            let area = batch
                .column_by_name("area_id")
                .and_then(|column| column.as_any().downcast_ref::<StringArray>());
            let memory = batch
                .column_by_name("memory_id")
                .and_then(|column| column.as_any().downcast_ref::<StringArray>());
            let owner = batch
                .column_by_name("owner_actor_id")
                .and_then(|column| column.as_any().downcast_ref::<StringArray>());
            let distance = batch
                .column_by_name("_distance")
                .and_then(|column| column.as_any().downcast_ref::<arrow_array::Float32Array>());
            let Some((tenant, area, memory, owner)) = tenant
                .zip(area)
                .zip(memory)
                .zip(owner)
                .map(|(((tenant, area), memory), owner)| (tenant, area, memory, owner))
            else {
                continue;
            };
            for index in 0..batch.num_rows() {
                let Ok(tenant_id) = tenant.value(index).parse() else {
                    continue;
                };
                let Ok(area_id) = area.value(index).parse() else {
                    continue;
                };
                let Ok(memory_id) = memory.value(index).parse() else {
                    continue;
                };
                hits.push(LanceHit {
                    tenant_id: TenantId::new(tenant_id),
                    area_id: AreaId::new(area_id),
                    memory_id: MemoryId::new(memory_id),
                    owner_actor_id: (!owner.is_null(index))
                        .then(|| owner.value(index).parse().ok())
                        .flatten(),
                    distance: distance.map_or(0.0, |values| values.value(index)),
                });
            }
        }
        Ok(hits)
    }
}

impl PgRepository {
    /// Starts an explicit aggressive investigation. The operation and trace
    /// are idempotent on the caller's tenant-scoped key.
    pub async fn start_aggressive_search(
        &self,
        intent: AggressiveIntent,
        budgets: SearchBudgets,
        idempotency_key: &str,
    ) -> Result<SearchTrace, ApplicationError> {
        let trace_id = Uuid::now_v7();
        let now = time::OffsetDateTime::now_utc();
        let trace = SearchTrace::start(intent.clone(), budgets, now, trace_id).map_err(|e| {
            ApplicationError::InvalidRequest {
                message: e.to_string(),
            }
        })?;
        let payload =
            serde_json::to_value(&trace).map_err(|_| ApplicationError::InternalUnexpected)?;
        let operation_id = self
            .enqueue_operation(
                intent.tenant_id,
                trace_id.into(),
                &serde_json::json!({"kind":"aggressive_search","trace":payload}),
                "aggressive-search",
                idempotency_key,
                3,
            )
            .await?;
        sqlx::query("INSERT INTO search_traces (trace_id,tenant_id,operation_id,actor_id,host_id,agent_identity_id,session_id,area_id,purpose,task,state,descriptor,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$13) ON CONFLICT (tenant_id,operation_id) DO NOTHING")
            .bind(trace_id).bind(intent.tenant_id.as_uuid()).bind(operation_id.as_uuid()).bind(intent.actor_id).bind(&intent.host_id).bind(intent.agent_identity_id.as_uuid()).bind(intent.session_id).bind(intent.area_id.as_uuid()).bind(&intent.purpose).bind(&intent.task).bind("queued").bind(&payload).bind(now)
            .execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        self.aggressive_search_trace(intent.tenant_id, operation_id)
            .await
    }

    pub async fn aggressive_search_trace(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
    ) -> Result<SearchTrace, ApplicationError> {
        let value = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT descriptor FROM search_traces WHERE tenant_id=$1 AND operation_id=$2",
        )
        .bind(tenant_id.as_uuid())
        .bind(operation_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?
        .ok_or(ApplicationError::OperationNotFound)?;
        serde_json::from_value(value).map_err(|_| ApplicationError::InternalUnexpected)
    }

    pub async fn persist_aggressive_trace(
        &self,
        tenant_id: TenantId,
        trace: &SearchTrace,
    ) -> Result<(), ApplicationError> {
        let descriptor =
            serde_json::to_value(trace).map_err(|_| ApplicationError::InternalUnexpected)?;
        let result = sqlx::query("UPDATE search_traces SET state=$3, descriptor=$4, updated_at=$5 WHERE tenant_id=$1 AND trace_id=$2")
            .bind(tenant_id.as_uuid()).bind(trace.trace_id).bind(format!("{:?}", trace.state).to_ascii_lowercase()).bind(&descriptor).bind(trace.updated_at).execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        for step in &trace.steps {
            let descriptor =
                serde_json::to_value(step).map_err(|_| ApplicationError::InternalUnexpected)?;
            sqlx::query("INSERT INTO search_trace_steps (trace_id,tenant_id,ordinal,kind,area_id,descriptor,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (trace_id,ordinal) DO UPDATE SET descriptor=EXCLUDED.descriptor")
                .bind(trace.trace_id).bind(tenant_id.as_uuid()).bind(step.ordinal as i32).bind(format!("{:?}", step.kind).to_ascii_lowercase()).bind(step.area_id.as_uuid()).bind(descriptor).bind(trace.updated_at).execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        }
        Ok(())
    }

    pub async fn cancel_aggressive_search(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
    ) -> Result<(), ApplicationError> {
        self.request_operation_cancel(tenant_id, operation_id).await
    }

    /// Returns only exact evidence links for already-selected Memory. The
    /// query never searches Source bodies; it follows canonical lineage from
    /// the selected Memory version to its immutable Source-version/chunk.
    pub async fn aggressive_source_evidence(
        &self,
        tenant_id: TenantId,
        memory_ids: &[MemoryId],
    ) -> Result<Vec<SourceEvidence>, ApplicationError> {
        if memory_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<Uuid> = memory_ids.iter().map(|id| id.as_uuid()).collect();
        let rows = sqlx::query(
            "SELECT mv.memory_id, sv.source_id, el.source_version_id, el.chunk_id, el.coordinate, c.content_hash, c.content FROM evidence_links el JOIN memory_versions mv ON mv.tenant_id=el.tenant_id AND mv.memory_version_id=el.memory_version_id JOIN source_versions sv ON sv.tenant_id=el.tenant_id AND sv.source_version_id=el.source_version_id LEFT JOIN chunks c ON c.tenant_id=el.tenant_id AND c.chunk_id=el.chunk_id WHERE el.tenant_id=$1 AND mv.memory_id=ANY($2) AND el.state IN ('attached','proposed') ORDER BY mv.memory_id, el.source_version_id, el.chunk_id, el.coordinate",
        )
        .bind(tenant_id.as_uuid())
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let source_version_id: Option<Uuid> = row.try_get("source_version_id").ok();
                let chunk_id: Option<Uuid> = row.try_get("chunk_id").ok();
                let source_id: Option<Uuid> = row.try_get("source_id").ok();
                let (Some(source_id), Some(source_version_id), Some(chunk_id)) =
                    (source_id, source_version_id, chunk_id)
                else {
                    return None;
                };
                Some(SourceEvidence {
                    citation: Citation::exact_source(
                        source_id.into(),
                        source_version_id.into(),
                        chunk_id.into(),
                        row.try_get::<Option<String>, _>("coordinate")
                            .ok()
                            .flatten()
                            .unwrap_or_default(),
                        row.try_get::<Option<String>, _>("content_hash")
                            .ok()
                            .flatten()
                            .unwrap_or_default(),
                    ),
                    content: row
                        .try_get::<Option<String>, _>("content")
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                })
            })
            .collect())
    }

    pub async fn approved_cross_map_targets(
        &self,
        tenant_id: TenantId,
        source_area_id: AreaId,
        limit: u32,
    ) -> Result<Vec<AreaId>, ApplicationError> {
        let rows = sqlx::query(
            "SELECT target_area_id FROM cross_map_mappings WHERE tenant_id=$1 AND source_area_id=$2 AND state='approved' ORDER BY target_area_id LIMIT $3",
        )
        .bind(tenant_id.as_uuid())
        .bind(source_area_id.as_uuid())
        .bind(limit.clamp(1, 100) as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(rows
            .into_iter()
            .map(|row| AreaId::new(row.get("target_area_id")))
            .collect())
    }

    /// Loads an already-authorized Area slice for the pure core graph
    /// traversal. Lifecycle and tenant filters happen before the slice is
    /// constructed, so the graph function cannot fan out through storage.
    pub async fn authorized_graph_slice(
        &self,
        tenant_id: TenantId,
        area_ids: &[AreaId],
        limit: u32,
    ) -> Result<AuthorizedGraphSlice, ApplicationError> {
        if area_ids.is_empty() {
            return Ok(AuthorizedGraphSlice::default());
        }
        let ids: Vec<Uuid> = area_ids.iter().map(|id| id.as_uuid()).collect();
        let max = limit.clamp(1, 500) as i64;
        let entity_rows = sqlx::query("SELECT entity_id,area_id,map_version_id,kind,state,origin,descriptor,version FROM entities WHERE tenant_id=$1 AND area_id=ANY($2) AND state IN ('active','proposed') ORDER BY entity_id LIMIT $3")
            .bind(tenant_id.as_uuid()).bind(&ids).bind(max).fetch_all(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        let entities = entity_rows
            .into_iter()
            .map(|row| Entity {
                entity_id: EntityId::new(row.get("entity_id")),
                tenant_id,
                area_id: AreaId::new(row.get("area_id")),
                map_version_id: MapVersionId::new(row.get("map_version_id")),
                kind: row.get("kind"),
                state: parse_entity_state(&row.get::<String, _>("state")),
                origin: parse_origin(&row.get::<String, _>("origin")),
                descriptor: row.get("descriptor"),
                version: row.get::<i64, _>("version") as u64,
            })
            .collect::<Vec<_>>();
        let relationship_rows = sqlx::query("SELECT relationship_id,area_id,map_version_id,source_entity_id,target_entity_id,relation_kind,state,origin,version FROM relationships WHERE tenant_id=$1 AND area_id=ANY($2) AND state IN ('active','proposed') ORDER BY relationship_id LIMIT $3")
            .bind(tenant_id.as_uuid()).bind(&ids).bind((max * 2).min(1000)).fetch_all(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        let relationships = relationship_rows
            .into_iter()
            .map(|row| Relationship {
                relationship_id: RelationshipId::new(row.get("relationship_id")),
                tenant_id,
                area_id: AreaId::new(row.get("area_id")),
                map_version_id: MapVersionId::new(row.get("map_version_id")),
                source_entity_id: EntityId::new(row.get("source_entity_id")),
                target_entity_id: EntityId::new(row.get("target_entity_id")),
                relation_kind: row.get("relation_kind"),
                state: parse_relationship_state(&row.get::<String, _>("state")),
                origin: parse_origin(&row.get::<String, _>("origin")),
                version: row.get::<i64, _>("version") as u64,
            })
            .collect();
        Ok(AuthorizedGraphSlice {
            entities,
            relationships,
        })
    }

    pub async fn validate_mcp_context(
        &self,
        tenant_id: TenantId,
        actor_id: Uuid,
        agent_identity_id: engrave_contracts::AgentIdentityId,
        session_id: Uuid,
        area_id: AreaId,
        role: &str,
    ) -> Result<(), ApplicationError> {
        let valid = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM actors a
              JOIN memberships m ON m.tenant_id=a.tenant_id AND m.actor_id=a.actor_id AND m.state='active'
              JOIN roles r ON r.tenant_id=m.tenant_id AND r.role_id=m.role_id AND r.state='active' AND r.role_key=$6
              JOIN agent_identities ai ON ai.tenant_id=a.tenant_id AND ai.agent_identity_id=$3 AND ai.state='active'
              JOIN areas ar ON ar.tenant_id=a.tenant_id AND ar.area_id=$5 AND ar.state='active'
              JOIN area_grants g ON g.tenant_id=a.tenant_id AND g.area_id=ar.area_id AND g.state='active'
                AND g.effective_from <= now() AND (g.effective_until IS NULL OR g.effective_until >= now())
                AND ((g.actor_id=a.actor_id) OR (g.agent_identity_id=ai.agent_identity_id))
                AND (g.session_id IS NULL OR g.session_id=$4)
              WHERE a.tenant_id=$1 AND a.actor_id=$2 AND a.state='active'
            )
            "#,
        )
        .bind(tenant_id.as_uuid()).bind(actor_id).bind(agent_identity_id.as_uuid())
        .bind(session_id).bind(area_id.as_uuid()).bind(role)
        .fetch_one(&self.pool).await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if !valid {
            return Err(ApplicationError::Forbidden);
        }
        Ok(())
    }

    /// Drives the durable Phase J workspace interview. This method only
    /// creates a Proposal at `submit`; it never grants an Area or publishes a
    /// Map. The caller must have already passed the MCP Rule gateway.
    #[allow(clippy::too_many_arguments)]
    pub async fn workspace_setup(
        &self,
        tenant_id: TenantId,
        actor_id: Uuid,
        agent_identity_id: engrave_contracts::AgentIdentityId,
        session_id: Uuid,
        host_id: &str,
        purpose: &str,
        area_id: AreaId,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, ApplicationError> {
        let action = args
            .get("action")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ApplicationError::InvalidRequest {
                message: "workspace_setup action is required".into(),
            })?;
        let idempotency_key = args
            .get("idempotency_key")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("workspace-setup:{session_id}"));
        let authorization = self
            .resolve_search_authorization(tenant_id, actor_id, &[], purpose.to_owned())
            .await?;
        let allowed = authorization.permitted_area_ids;
        let area_options = self.workspace_area_options(tenant_id, &allowed).await?;
        let selected = args
            .get("selected_area_ids")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_else(|| vec![serde_json::json!(area_id.as_uuid())]);
        let selected_ids = selected
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| ApplicationError::InvalidRequest {
                        message: "selected_area_ids must contain UUID strings".into(),
                    })
                    .and_then(|value| {
                        Uuid::parse_str(value).map_err(|_| ApplicationError::InvalidRequest {
                            message: "selected_area_ids must contain UUID strings".into(),
                        })
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if selected_ids
            .iter()
            .any(|selected| !allowed.contains(&AreaId::new(*selected)))
        {
            return Err(ApplicationError::Forbidden);
        }
        let requested = args
            .get("requested_areas")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        let draft = args
            .get("ontology_draft")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        if action == "begin" {
            if let Some(existing) = sqlx::query(
                "SELECT * FROM workspace_interviews WHERE tenant_id=$1 AND idempotency_key=$2",
            )
            .bind(tenant_id.as_uuid())
            .bind(&idempotency_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })? {
                if existing.get::<Uuid, _>("actor_id") != actor_id
                    || existing.get::<Uuid, _>("agent_identity_id") != agent_identity_id.as_uuid()
                    || existing.get::<Uuid, _>("session_id") != session_id
                {
                    return Err(ApplicationError::Forbidden);
                }
                let mut value = workspace_interview_json(&existing);
                value["authorized_area_ids"] =
                    serde_json::json!(allowed.iter().map(|id| id.as_uuid()).collect::<Vec<_>>());
                value["area_options"] = area_options.clone();
                value["request_new_area"] = serde_json::json!(true);
                return Ok(value);
            }
            let interview_id = Uuid::now_v7();
            sqlx::query("INSERT INTO workspace_interviews (interview_id,tenant_id,actor_id,agent_identity_id,session_id,host_id,purpose,state,selected_area_ids,requested_areas,ontology_draft,idempotency_key) VALUES ($1,$2,$3,$4,$5,$6,$7,'collecting',$8,$9,$10,$11)")
                .bind(interview_id).bind(tenant_id.as_uuid()).bind(actor_id).bind(agent_identity_id.as_uuid()).bind(session_id).bind(host_id).bind(purpose).bind(serde_json::json!(selected_ids)).bind(requested).bind(draft).bind(&idempotency_key)
                .execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
            let row = sqlx::query("SELECT * FROM workspace_interviews WHERE interview_id=$1")
                .bind(interview_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
            let mut value = workspace_interview_json(&row);
            value["authorized_area_ids"] =
                serde_json::json!(allowed.iter().map(|id| id.as_uuid()).collect::<Vec<_>>());
            value["area_options"] = area_options;
            value["request_new_area"] = serde_json::json!(true);
            return Ok(value);
        }

        let interview_id = Uuid::parse_str(
            args.get("interview_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| ApplicationError::InvalidRequest {
                    message: "interview_id is required after begin".into(),
                })?,
        )
        .map_err(|_| ApplicationError::InvalidRequest {
            message: "interview_id must be a UUID".into(),
        })?;
        let row = sqlx::query(
            "SELECT * FROM workspace_interviews WHERE tenant_id=$1 AND interview_id=$2",
        )
        .bind(tenant_id.as_uuid())
        .bind(interview_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?
        .ok_or(ApplicationError::Forbidden)?;
        if row.get::<Uuid, _>("actor_id") != actor_id
            || row.get::<Uuid, _>("agent_identity_id") != agent_identity_id.as_uuid()
            || row.get::<Uuid, _>("session_id") != session_id
        {
            return Err(ApplicationError::Forbidden);
        }
        let state: String = row.get("state");
        match action {
            "draft" => {
                if !matches!(state.as_str(), "collecting" | "awaiting_confirmation") {
                    return Err(ApplicationError::InvalidRequest {
                        message: "interview is not accepting a draft".into(),
                    });
                }
                sqlx::query("UPDATE workspace_interviews SET state='awaiting_confirmation',ontology_draft=$3,requested_areas=$4,selected_area_ids=$5,version=version+1,updated_at=now() WHERE tenant_id=$1 AND interview_id=$2")
                    .bind(tenant_id.as_uuid()).bind(interview_id).bind(draft).bind(requested).bind(serde_json::json!(selected_ids)).execute(&self.pool).await
                    .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
            }
            "confirm" => {
                if state == "confirmed" && row.get::<bool, _>("confirmed") {
                    return Ok(workspace_interview_json(&row));
                }
                if state != "awaiting_confirmation"
                    || args.get("confirmed").and_then(serde_json::Value::as_bool) != Some(true)
                {
                    return Err(ApplicationError::InvalidRequest {
                        message: "explicit confirmation is required".into(),
                    });
                }
                sqlx::query("UPDATE workspace_interviews SET state='confirmed',confirmed=true,version=version+1,updated_at=now() WHERE tenant_id=$1 AND interview_id=$2")
                    .bind(tenant_id.as_uuid()).bind(interview_id).execute(&self.pool).await
                    .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
            }
            "submit" => {
                if state == "submitted" {
                    return Ok(workspace_interview_json(&row));
                }
                if state != "confirmed" || !row.get::<bool, _>("confirmed") {
                    return Err(ApplicationError::InvalidRequest {
                        message: "interview must be explicitly confirmed before submit".into(),
                    });
                }
                let proposal_id = Uuid::now_v7();
                let payload = serde_json::json!({"interview_id":interview_id,"ontology_draft":row.get::<serde_json::Value, _>("ontology_draft"),"selected_area_ids":row.get::<serde_json::Value, _>("selected_area_ids"),"requested_areas":row.get::<serde_json::Value, _>("requested_areas"),"actor_id":actor_id,"agent_identity_id":agent_identity_id,"session_id":session_id});
                let mut tx = self.pool.begin().await.map_err(|_| {
                    ApplicationError::DependencyUnavailable {
                        dependency: "postgres",
                    }
                })?;
                let changed = sqlx::query("UPDATE workspace_interviews SET state='submitted',proposal_id=$3,version=version+1,updated_at=now() WHERE tenant_id=$1 AND interview_id=$2 AND state='confirmed' AND confirmed=true")
                    .bind(tenant_id.as_uuid()).bind(interview_id).bind(proposal_id).execute(&mut *tx).await
                    .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
                if changed.rows_affected() == 0 {
                    return Err(ApplicationError::InvalidRequest {
                        message: "interview was already submitted or changed".into(),
                    });
                }
                sqlx::query("INSERT INTO proposals (proposal_id,tenant_id,area_id,state,origin,kind,payload,version) VALUES ($1,$2,$3,'pending','workspace_setup','map_area', $4, 1)")
                    .bind(proposal_id).bind(tenant_id.as_uuid()).bind(area_id.as_uuid()).bind(payload).execute(&mut *tx).await
                    .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
                tx.commit()
                    .await
                    .map_err(|_| ApplicationError::DependencyUnavailable {
                        dependency: "postgres",
                    })?;
            }
            "inspect" => {}
            "cancel" => {
                if state == "cancelled" {
                    return Ok(workspace_interview_json(&row));
                }
                if state == "submitted" {
                    return Err(ApplicationError::InvalidRequest {
                        message: "interview cannot be cancelled in its current state".into(),
                    });
                }
                sqlx::query("UPDATE workspace_interviews SET state='cancelled',version=version+1,updated_at=now() WHERE tenant_id=$1 AND interview_id=$2")
                    .bind(tenant_id.as_uuid()).bind(interview_id).execute(&self.pool).await
                    .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
            }
            _ => {
                return Err(ApplicationError::InvalidRequest {
                    message: "action must be begin, draft, confirm, submit, inspect, or cancel"
                        .into(),
                })
            }
        }
        let updated = sqlx::query(
            "SELECT * FROM workspace_interviews WHERE tenant_id=$1 AND interview_id=$2",
        )
        .bind(tenant_id.as_uuid())
        .bind(interview_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        let mut value = workspace_interview_json(&updated);
        value["authorized_area_ids"] =
            serde_json::json!(allowed.iter().map(|id| id.as_uuid()).collect::<Vec<_>>());
        value["area_options"] = area_options;
        value["request_new_area"] = serde_json::json!(true);
        Ok(value)
    }

    async fn workspace_area_options(
        &self,
        tenant_id: TenantId,
        allowed: &std::collections::BTreeSet<AreaId>,
    ) -> Result<serde_json::Value, ApplicationError> {
        let ids: Vec<Uuid> = allowed.iter().map(|id| id.as_uuid()).collect();
        let rows = sqlx::query(
            "SELECT area_id, slug FROM areas WHERE tenant_id=$1 AND state='active' AND area_id=ANY($2) ORDER BY slug, area_id",
        )
        .bind(tenant_id.as_uuid())
        .bind(ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(serde_json::Value::Array(
            rows.into_iter()
                .map(|row| {
                    serde_json::json!({
                        "area_id": row.get::<Uuid, _>("area_id"),
                        "label": row.get::<String, _>("slug"),
                    })
                })
                .collect(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn queue_connector_source(
        &self,
        tenant_id: TenantId,
        area_id: AreaId,
        connector: &str,
        external_id: &str,
        title: &str,
        content: &str,
        permissions: &serde_json::Value,
        idempotency_key: &str,
    ) -> Result<OperationId, ApplicationError> {
        let source_id = Uuid::now_v7();
        let source_version_id = Uuid::now_v7();
        let checksum = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        let source_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sources (source_id, tenant_id, area_id, state, title, created_at, updated_at, connector_name, external_id, connector_permissions)
            VALUES ($1,$2,$3,'queued',$4,now(),now(),$5,$6,$7)
            ON CONFLICT (tenant_id, connector_name, external_id) WHERE connector_name IS NOT NULL AND external_id IS NOT NULL
            DO UPDATE SET title=EXCLUDED.title, area_id=EXCLUDED.area_id, connector_permissions=EXCLUDED.connector_permissions, updated_at=now()
            RETURNING source_id
            "#
        ).bind(source_id).bind(tenant_id.as_uuid()).bind(area_id.as_uuid()).bind(title).bind(connector).bind(external_id).bind(permissions)
        .fetch_one(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        let next_version = sqlx::query_scalar::<_, i64>("SELECT COALESCE(max(version_number),0)+1 FROM source_versions WHERE tenant_id=$1 AND source_id=$2 AND checksum <> $3")
            .bind(tenant_id.as_uuid()).bind(source_id).bind(&checksum).fetch_one(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        let existing = sqlx::query_scalar::<_, Uuid>("SELECT source_version_id FROM source_versions WHERE tenant_id=$1 AND source_id=$2 AND checksum=$3")
            .bind(tenant_id.as_uuid()).bind(source_id).bind(&checksum).fetch_optional(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        let source_version_id = if let Some(existing) = existing {
            existing
        } else {
            sqlx::query("UPDATE source_versions SET state='superseded' WHERE tenant_id=$1 AND source_id=$2 AND state='current'").bind(tenant_id.as_uuid()).bind(source_id).execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
            sqlx::query("INSERT INTO source_versions (source_version_id,tenant_id,source_id,version_number,state,object_key,media_type,byte_size,checksum,created_at) VALUES ($1,$2,$3,$4,'current',$5,'text/plain',$6,$7,now())")
                .bind(source_version_id).bind(tenant_id.as_uuid()).bind(source_id).bind(next_version).bind(format!("tenants/{}/sources/{source_id}/connector/{external_id}/{next_version}", tenant_id.as_uuid())).bind(content.len() as i64).bind(&checksum).execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
            source_version_id
        };
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        let operation_id = OperationId::new_v7();
        self.enqueue_operation(tenant_id, operation_id, &serde_json::json!({"kind":"connector_ingest","connector":connector,"external_id":external_id,"source_id":source_id,"source_version_id":source_version_id,"content":content,"title":title}), "connector-ingest", idempotency_key, 5).await
    }
    pub async fn connect(database_url: &str) -> Result<Self, ApplicationError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn ping(&self) -> Result<(), ApplicationError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Persists a draft Rule and its immutable version. Activation is a
    /// separate, admission-gated operation so a test harness can run first.
    pub async fn create_rule(&self, rule: &Rule) -> Result<(), ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        sqlx::query("INSERT INTO rules (rule_id, tenant_id, area_id, state, environment, owner_actor_id) VALUES ($1,$2,$3,$4,$5,$6)")
            .bind(rule.id.as_uuid()).bind(rule.scope.tenant_id.as_uuid())
            .bind(rule.scope.area_ids.iter().next().map(|id| id.as_uuid()))
            .bind(match rule.state { engrave_contracts::RuleState::Draft => "draft", engrave_contracts::RuleState::Active => "active", engrave_contracts::RuleState::Superseded => "superseded", engrave_contracts::RuleState::Disabled => "disabled" })
            .bind(rule.scope.environment.as_deref().unwrap_or("default"))
            .bind(rule.scope.actor_ids.iter().next())
            .execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        sqlx::query("INSERT INTO rule_versions (rule_version_id, tenant_id, rule_id, version_number, effect, condition, rationale, scope, evaluation_points, priority, locked, effective_from, effective_until) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)")
            .bind(rule.version_id.as_uuid()).bind(rule.scope.tenant_id.as_uuid()).bind(rule.id.as_uuid()).bind(rule.version_number as i64)
            .bind(match rule.effect { engrave_contracts::RuleEffect::Allow => "allow", engrave_contracts::RuleEffect::Warn => "warn", engrave_contracts::RuleEffect::RequireApproval => "require_approval", engrave_contracts::RuleEffect::Block => "block" })
            .bind(serde_json::to_value(&rule.conditions).unwrap_or_default()).bind(&rule.rationale).bind(serde_json::to_value(&rule.scope).unwrap_or_default()).bind(serde_json::to_value(&rule.evaluation_points).unwrap_or_default()).bind(rule.priority).bind(rule.locked).bind(rule.effective_from).bind(rule.effective_until)
            .execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        sqlx::query(
            "UPDATE rules SET current_version_id = $1 WHERE tenant_id = $2 AND rule_id = $3",
        )
        .bind(rule.version_id.as_uuid())
        .bind(rule.scope.tenant_id.as_uuid())
        .bind(rule.id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })
    }

    pub async fn activate_rule(
        &self,
        tenant_id: TenantId,
        rule_id: engrave_contracts::RuleId,
        expected_version: u64,
        idempotency_key: &str,
    ) -> Result<(), ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        let replay = sqlx::query_scalar::<_, String>("SELECT request_hash FROM idempotency_keys WHERE tenant_id = $1 AND scope = 'rule-activation' AND key = $2")
            .bind(tenant_id.as_uuid()).bind(idempotency_key).fetch_optional(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if replay.is_some() {
            return Ok(());
        }
        sqlx::query("INSERT INTO idempotency_keys (tenant_id, scope, key, request_hash, created_at) VALUES ($1,'rule-activation',$2,$3,now())")
            .bind(tenant_id.as_uuid()).bind(idempotency_key).bind(rule_id.as_uuid().to_string()).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        let bumped = sqlx::query("UPDATE rules SET version = version + 1 WHERE tenant_id = $1 AND rule_id = $2 AND version = $3 AND state = 'draft'")
            .bind(tenant_id.as_uuid()).bind(rule_id.as_uuid()).bind(expected_version as i64).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if bumped.rows_affected() == 0 {
            return Err(ApplicationError::VersionConflict {
                resource: "rule",
                current_version: expected_version,
            });
        }
        sqlx::query("SELECT set_config('app.rule_admission', 'approved', true)")
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        sqlx::query("UPDATE rules SET state = 'active' WHERE tenant_id = $1 AND rule_id = $2 AND state = 'draft'").bind(tenant_id.as_uuid()).bind(rule_id.as_uuid()).execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })
    }

    /// Loads the active immutable versions for one tenant. The API performs
    /// this at the authorization boundary on every request so a Rule change
    /// cannot remain resident in a process-local cache past its admission.
    pub async fn active_rules(&self, tenant_id: TenantId) -> Result<Vec<Rule>, ApplicationError> {
        let rows = sqlx::query("SELECT r.rule_id, r.environment, rv.rule_version_id, rv.version_number, rv.effect, rv.condition, rv.rationale, rv.scope, rv.evaluation_points, rv.priority, rv.locked, rv.effective_from, rv.effective_until FROM rules r JOIN rule_versions rv ON rv.rule_version_id = r.current_version_id AND rv.tenant_id = r.tenant_id WHERE r.tenant_id = $1 AND r.state = 'active'")
            .bind(tenant_id.as_uuid()).fetch_all(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        rows.into_iter()
            .map(|row| {
                let mut scope: engrave_core::RuleScope = serde_json::from_value(row.get("scope"))
                    .map_err(|_| {
                    ApplicationError::InvalidRequest {
                        message: "stored Rule scope is invalid".into(),
                    }
                })?;
                scope.tenant_id = tenant_id;
                scope.environment = Some(row.get::<String, _>("environment"));
                Ok(Rule {
                    id: engrave_contracts::RuleId::new(row.get("rule_id")),
                    version_id: engrave_contracts::RuleVersionId::new(row.get("rule_version_id")),
                    version_number: row.get::<i64, _>("version_number") as u64,
                    scope,
                    conditions: serde_json::from_value(row.get("condition")).map_err(|_| {
                        ApplicationError::InvalidRequest {
                            message: "stored Rule conditions are invalid".into(),
                        }
                    })?,
                    evaluation_points: serde_json::from_value(row.get("evaluation_points"))
                        .map_err(|_| ApplicationError::InvalidRequest {
                            message: "stored Rule evaluation points are invalid".into(),
                        })?,
                    priority: row.get("priority"),
                    locked: row.get("locked"),
                    effect: serde_json::from_value(serde_json::Value::String(row.get("effect")))
                        .map_err(|_| ApplicationError::InvalidRequest {
                            message: "stored Rule effect is invalid".into(),
                        })?,
                    rationale: row.get("rationale"),
                    state: engrave_contracts::RuleState::Active,
                    effective_from: row.get("effective_from"),
                    effective_until: row.get("effective_until"),
                })
            })
            .collect()
    }

    pub async fn record_rule_decision(
        &self,
        tenant_id: TenantId,
        decision: &RuleDecision,
        request_id: &str,
        outcome: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO rule_decisions (rule_decision_id, tenant_id, rule_id, rule_version_id, actor_id, area_id, purpose, evaluation_point, effect, rationale, next_action, envelope, outcome, request_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
            .bind(Uuid::now_v7()).bind(tenant_id.as_uuid()).bind((decision.rule_id.as_uuid() != Uuid::nil()).then_some(decision.rule_id.as_uuid())).bind((decision.rule_version.as_uuid() != Uuid::nil()).then_some(decision.rule_version.as_uuid())).bind(decision.actor_id).bind(decision.envelope.allowed_area_ids.iter().next().map(|id| id.as_uuid())).bind(&decision.purpose).bind(format!("{:?}", decision.evaluation_point).to_ascii_lowercase()).bind(format!("{:?}", decision.effect).to_ascii_lowercase()).bind(&decision.rationale).bind(&decision.next_action).bind(serde_json::to_value(&decision.envelope).unwrap_or_default()).bind(outcome).bind(request_id)
            .execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_mcp_decision(
        &self,
        tenant_id: TenantId,
        actor_id: Uuid,
        host_id: &str,
        agent_identity_id: engrave_contracts::AgentIdentityId,
        session_id: Uuid,
        area_id: AreaId,
        decision: &RuleDecision,
        outcome: &str,
        argument_hash: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO rule_decisions (rule_decision_id,tenant_id,rule_id,rule_version_id,actor_id,host_id,agent_identity_id,session_id,area_id,purpose,evaluation_point,effect,rationale,next_action,envelope,outcome,request_id,idempotency_key) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)")
            .bind(Uuid::now_v7()).bind(tenant_id.as_uuid()).bind((decision.rule_id.as_uuid() != Uuid::nil()).then_some(decision.rule_id.as_uuid())).bind((decision.rule_version.as_uuid() != Uuid::nil()).then_some(decision.rule_version.as_uuid()))
            .bind(actor_id).bind(host_id).bind(agent_identity_id.as_uuid()).bind(session_id).bind(area_id.as_uuid()).bind(&decision.purpose)
            .bind(format!("{:?}", decision.evaluation_point).to_ascii_lowercase())
            .bind(format!("{:?}", decision.effect).to_ascii_lowercase()).bind(&decision.rationale).bind(&decision.next_action)
            .bind(serde_json::json!({"policy":decision.envelope,"argument_hash":argument_hash})).bind(outcome).bind(session_id.to_string()).bind(argument_hash)
            .execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(())
    }

    /// Resolves retrieval scope from canonical tenant membership and Area
    /// grants. Requested Areas are only a narrowing hint; they never grant
    /// access. Enterprise admins receive all active tenant Areas.
    pub async fn resolve_search_authorization(
        &self,
        tenant_id: TenantId,
        actor_id: Uuid,
        requested_area_ids: &[Uuid],
        purpose: String,
    ) -> Result<AuthorizationContext, ApplicationError> {
        let actor_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM actors WHERE tenant_id = $1 AND actor_id = $2 AND state = 'active')",
        )
        .bind(tenant_id.as_uuid())
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if !actor_exists {
            return Err(ApplicationError::Forbidden);
        }
        let is_admin = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM memberships m JOIN roles r ON r.tenant_id = m.tenant_id AND r.role_id = m.role_id WHERE m.tenant_id = $1 AND m.actor_id = $2 AND m.state = 'active' AND r.state = 'active' AND r.role_key = 'enterprise_admin')",
        )
        .bind(tenant_id.as_uuid())
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        let permitted = if is_admin {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT area_id FROM areas WHERE tenant_id = $1 AND state = 'active'",
            )
            .bind(tenant_id.as_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?
        } else {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT area_id FROM area_grants WHERE tenant_id = $1 AND actor_id = $2 AND state = 'active' AND effective_from <= now() AND (effective_until IS NULL OR effective_until >= now())",
            )
            .bind(tenant_id.as_uuid())
            .bind(actor_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?
        };
        let permitted = permitted
            .into_iter()
            .filter(|area_id| requested_area_ids.is_empty() || requested_area_ids.contains(area_id))
            .map(AreaId::new)
            .collect();
        Ok(AuthorizationContext {
            tenant_id,
            actor_id: Some(actor_id),
            permitted_area_ids: permitted,
            role: if is_admin {
                ActorRole::EnterpriseAdmin
            } else {
                ActorRole::NormalUser
            },
            purpose,
        })
    }

    /// Loads canonical active Memories and deterministically materializes the
    /// rebuildable LanceDB rows used by the worker's indexing boundary.
    pub async fn retrieval_projection_rows(
        &self,
        tenant_id: TenantId,
        identity: &ProjectionIdentity,
    ) -> Result<Vec<LanceProjectionRow>, ApplicationError> {
        let rows = sqlx::query(
            "SELECT m.memory_id, m.area_id, mv.claim, mv.scope, mv.owner_actor_id FROM memories m JOIN memory_versions mv ON mv.tenant_id = m.tenant_id AND mv.memory_version_id = m.current_version_id WHERE m.tenant_id = $1 AND m.state = 'active' AND mv.state = 'current'",
        )
        .bind(tenant_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        let mut projected = Vec::with_capacity(rows.len());
        if std::env::var("ENGRAVE_EMBEDDING_PROFILE").as_deref() == Ok("voyage-3-lite") {
            let client = VoyageEmbeddingClient::from_env().map_err(|_| {
                ApplicationError::DependencyUnavailable {
                    dependency: "voyage",
                }
            })?;
            let claims: Vec<String> = rows.iter().map(|row| row.get("claim")).collect();
            let values = client.embed_batch(&claims).await.map_err(|_| {
                ApplicationError::DependencyUnavailable {
                    dependency: "voyage",
                }
            })?;
            for (row, values) in rows.into_iter().zip(values) {
                let vector = engrave_core::EmbeddingVector::normalized(values, identity.dimension)
                    .map_err(|_| ApplicationError::InvalidRequest {
                        message: "unable to normalize Voyage projection".into(),
                    })?;
                projected.push(LanceProjectionRow {
                    tenant_id,
                    area_id: AreaId::new(row.get("area_id")),
                    memory_id: MemoryId::new(row.get("memory_id")),
                    owner_actor_id: row.get("owner_actor_id"),
                    scope: row.get("scope"),
                    state: "current".into(),
                    identity: identity.clone(),
                    vector,
                });
            }
        } else if std::env::var("ENGRAVE_EMBEDDING_PROFILE").as_deref() == Ok("openai-large") {
            let client = OpenAiEmbeddingClient::from_env().map_err(|_| {
                ApplicationError::DependencyUnavailable {
                    dependency: "openai",
                }
            })?;
            let claims: Vec<String> = rows.iter().map(|row| row.get("claim")).collect();
            let values = client.embed_batch(&claims).await.map_err(|_| {
                ApplicationError::DependencyUnavailable {
                    dependency: "openai",
                }
            })?;
            for (row, values) in rows.into_iter().zip(values) {
                let vector = engrave_core::EmbeddingVector::normalized(values, identity.dimension)
                    .map_err(|_| ApplicationError::InvalidRequest {
                        message: "unable to normalize OpenAI projection".into(),
                    })?;
                projected.push(LanceProjectionRow {
                    tenant_id,
                    area_id: AreaId::new(row.get("area_id")),
                    memory_id: MemoryId::new(row.get("memory_id")),
                    owner_actor_id: row.get("owner_actor_id"),
                    scope: row.get("scope"),
                    state: "current".into(),
                    identity: identity.clone(),
                    vector,
                });
            }
        } else {
            let provider = DeterministicEmbeddingProvider::new(identity.clone());
            for row in rows {
                let claim: String = row.get("claim");
                projected.push(LanceProjectionRow {
                    tenant_id,
                    area_id: AreaId::new(row.get("area_id")),
                    memory_id: MemoryId::new(row.get("memory_id")),
                    owner_actor_id: row.get("owner_actor_id"),
                    scope: row.get("scope"),
                    state: "current".into(),
                    identity: identity.clone(),
                    vector: provider.embed(&claim).map_err(|_| {
                        ApplicationError::InvalidRequest {
                            message: "unable to embed canonical claim".into(),
                        }
                    })?,
                });
            }
        }
        Ok(projected)
    }

    /// Authorization-first PostgreSQL lexical retrieval. The tenant, Area,
    /// lifecycle, applicability, role, and private-owner predicates are in
    /// the candidate query itself; PostgreSQL never ranks an ineligible row.
    pub async fn search_lexical(
        &self,
        request: &SearchRequest,
    ) -> Result<Vec<LexicalHit>, ApplicationError> {
        let area_ids: Vec<Uuid> = request
            .authorization
            .permitted_area_ids
            .iter()
            .map(|area_id| area_id.as_uuid())
            .collect();
        if area_ids.is_empty() || request.query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let role = match request.authorization.role {
            ActorRole::NormalUser => "normal_user",
            ActorRole::EnterpriseAdmin => "enterprise_admin",
        };
        let rows = sqlx::query(
            r#"
            WITH query_terms AS (
                SELECT DISTINCT term
                FROM regexp_split_to_table(lower($1), '[^[:alnum:]]+') AS term
                WHERE length(term) > 2
            ), eligible AS (
                SELECT mv.memory_id, mv.tenant_id, m.area_id, mv.claim,
                       mv.reason, mv.evidence, mv.scope, mv.owner_actor_id,
                       mv.applies_when,
                       mv.valid_from, mv.valid_until,
                       regexp_split_to_array(
                           lower(concat_ws(' ', mv.claim, mv.reason, mv.applies_when)),
                           '[^[:alnum:]]+'
                       ) AS terms
                FROM memory_versions mv
                JOIN memories m ON m.tenant_id = mv.tenant_id
                               AND m.current_version_id = mv.memory_version_id
                WHERE mv.tenant_id = $2
                  AND m.area_id = ANY($3)
                  AND m.state = 'active'
                  AND mv.state = 'current'
                  AND (mv.valid_from IS NULL OR mv.valid_from <= $5)
                  AND (mv.valid_until IS NULL OR mv.valid_until >= $5)
                  AND (mv.applies_when = 'always' OR mv.applies_when = $6)
                  AND mv.search_document @@ websearch_to_tsquery('simple', $1)
                  AND (
                      (mv.scope LIKE 'personal%' OR mv.scope LIKE 'private%')
                          AND mv.owner_actor_id = $4
                      OR (mv.scope NOT LIKE 'personal%'
                          AND mv.scope NOT LIKE 'private%'
                          AND mv.scope NOT LIKE 'enterprise%')
                      OR ($7 = 'enterprise_admin' AND mv.scope LIKE 'enterprise%')
                  )
            ), stats AS (
                SELECT count(*)::float8 AS total,
                       coalesce(avg(cardinality(terms)), 1.0)::float8 AS avg_len
                FROM eligible
            ), term_stats AS (
                SELECT qt.term,
                       count(*) FILTER (WHERE e.terms @> ARRAY[qt.term])::float8 AS df
                FROM query_terms qt CROSS JOIN eligible e
                GROUP BY qt.term
            )
            SELECT e.memory_id, e.tenant_id, e.area_id, e.claim, e.reason,
                   e.evidence, e.scope, e.owner_actor_id, e.applies_when,
                   e.valid_from, e.valid_until,
                   (
                       SELECT coalesce(sum(
                           ln(((s.total - ts.df + 0.5) / (ts.df + 0.5)) + 1.0)
                           * ((cardinality(array_positions(e.terms, qt.term)) * 2.2)
                              / (cardinality(array_positions(e.terms, qt.term))
                                 + 1.2 * (0.25 + 0.75 * cardinality(e.terms) / s.avg_len)))
                       ), 0.0)
                       FROM query_terms qt
                       JOIN term_stats ts ON ts.term = qt.term
                       WHERE e.terms @> ARRAY[qt.term]
                   ) AS bm25_score
            FROM eligible e CROSS JOIN stats s
            ORDER BY bm25_score DESC, e.memory_id
            LIMIT $8
            "#,
        )
        .bind(&request.query)
        .bind(request.authorization.tenant_id.as_uuid())
        .bind(&area_ids)
        .bind(request.authorization.actor_id)
        .bind(request.now)
        .bind(&request.authorization.purpose)
        .bind(role)
        .bind(request.entry_limit.min(30) as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;

        let mut hits = rows
            .into_iter()
            .map(|row| {
                let scope: String = row.get("scope");
                let evidence = row
                    .get::<serde_json::Value, _>("evidence")
                    .as_array()
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|value| value.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let record = MemoryRecord {
                    tenant_id: TenantId::new(row.get("tenant_id")),
                    area_id: AreaId::new(row.get("area_id")),
                    memory_id: MemoryId::new(row.get("memory_id")),
                    claim: row.get("claim"),
                    reason: row.get("reason"),
                    evidence,
                    visibility: if scope.starts_with("personal") || scope.starts_with("private") {
                        Visibility::Private
                    } else if scope.starts_with("enterprise") {
                        Visibility::Enterprise
                    } else {
                        Visibility::Area
                    },
                    owner_actor_id: row.get("owner_actor_id"),
                    approved: true,
                    current: true,
                    archived: false,
                    superseded: false,
                    expired: false,
                    valid_from: row.get("valid_from"),
                    valid_until: row.get("valid_until"),
                    applies_when: row.get("applies_when"),
                    contradiction_warning: None,
                    lineage_warning: None,
                };
                Ok::<_, ApplicationError>(LexicalHit {
                    record,
                    score: row.get::<f64, _>("bm25_score") as f32,
                    rank: 0,
                    reason: "live PostgreSQL BM25 lexical match".into(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let floor = hits.first().map_or(0.0, |hit| hit.score * 0.35);
        hits.retain(|hit| hit.score >= floor);
        for (index, hit) in hits.iter_mut().enumerate() {
            hit.rank = index + 1;
        }
        Ok(hits)
    }

    /// Rehydrates vector candidates from canonical PostgreSQL rows and
    /// reapplies the authorization predicate. LanceDB metadata is a filter,
    /// never the source of truth for claim text or lifecycle.
    pub async fn rehydrate_dense_hits(
        &self,
        request: &SearchRequest,
        hits: &[LanceHit],
        identity: &ProjectionIdentity,
    ) -> Result<Vec<DenseHit>, ApplicationError> {
        if hits.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<Uuid> = hits.iter().map(|hit| hit.memory_id.as_uuid()).collect();
        let areas: Vec<Uuid> = request
            .authorization
            .permitted_area_ids
            .iter()
            .map(|area| area.as_uuid())
            .collect();
        let role = match request.authorization.role {
            ActorRole::NormalUser => "normal_user",
            ActorRole::EnterpriseAdmin => "enterprise_admin",
        };
        let rows = sqlx::query(
            r#"SELECT mv.memory_id, mv.tenant_id, m.area_id, mv.claim,
                      mv.reason, mv.evidence, mv.scope, mv.owner_actor_id,
                      mv.applies_when, mv.valid_from, mv.valid_until
               FROM memory_versions mv
               JOIN memories m ON m.tenant_id = mv.tenant_id
                              AND m.current_version_id = mv.memory_version_id
               WHERE mv.memory_id = ANY($1)
                 AND mv.tenant_id = $2
                 AND m.area_id = ANY($3)
                 AND m.state = 'active'
                 AND mv.state = 'current'
                 AND (mv.valid_from IS NULL OR mv.valid_from <= $5)
                 AND (mv.valid_until IS NULL OR mv.valid_until >= $5)
                 AND (mv.applies_when = 'always' OR mv.applies_when = $6)
                 AND (
                     (mv.scope LIKE 'personal%' OR mv.scope LIKE 'private%')
                         AND mv.owner_actor_id = $4
                     OR (mv.scope NOT LIKE 'personal%'
                         AND mv.scope NOT LIKE 'private%'
                         AND mv.scope NOT LIKE 'enterprise%')
                     OR ($7 = 'enterprise_admin' AND mv.scope LIKE 'enterprise%')
                 )"#,
        )
        .bind(&ids)
        .bind(request.authorization.tenant_id.as_uuid())
        .bind(&areas)
        .bind(request.authorization.actor_id)
        .bind(request.now)
        .bind(&request.authorization.purpose)
        .bind(role)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;

        let mut by_id = std::collections::BTreeMap::new();
        for row in rows {
            let scope: String = row.get("scope");
            let evidence = row
                .get::<serde_json::Value, _>("evidence")
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            by_id.insert(
                MemoryId::new(row.get("memory_id")),
                MemoryRecord {
                    tenant_id: TenantId::new(row.get("tenant_id")),
                    area_id: AreaId::new(row.get("area_id")),
                    memory_id: MemoryId::new(row.get("memory_id")),
                    claim: row.get("claim"),
                    reason: row.get("reason"),
                    evidence,
                    visibility: if scope.starts_with("personal") || scope.starts_with("private") {
                        Visibility::Private
                    } else if scope.starts_with("enterprise") {
                        Visibility::Enterprise
                    } else {
                        Visibility::Area
                    },
                    owner_actor_id: row.get("owner_actor_id"),
                    approved: true,
                    current: true,
                    archived: false,
                    superseded: false,
                    expired: false,
                    valid_from: row.get("valid_from"),
                    valid_until: row.get("valid_until"),
                    applies_when: row.get("applies_when"),
                    contradiction_warning: None,
                    lineage_warning: None,
                },
            );
        }
        let mut dense = Vec::new();
        for (rank, hit) in hits.iter().enumerate() {
            if let Some(record) = by_id.remove(&hit.memory_id) {
                dense.push(DenseHit {
                    record,
                    similarity: (1.0 - hit.distance * hit.distance / 2.0).clamp(-1.0, 1.0),
                    rank: rank + 1,
                    identity: identity.clone(),
                });
            }
        }
        Ok(dense)
    }

    /// Persists a candidate only. No Memory row is touched, which keeps
    /// ingestion/session-end capture safe even when admission policy changes.
    pub async fn create_memory_proposal(
        &self,
        tenant_id: TenantId,
        proposal_id: Uuid,
        area_id: Uuid,
        origin: &str,
        kind: &str,
        payload: &serde_json::Value,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO proposals (proposal_id, tenant_id, area_id, state, origin, kind, payload, version) VALUES ($1, $2, $3, 'pending', $4, $5, $6, 1) ON CONFLICT (proposal_id) DO NOTHING")
            .bind(proposal_id).bind(tenant_id.as_uuid()).bind(area_id).bind(origin).bind(kind).bind(payload)
            .execute(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(())
    }

    /// Approves a proposal with compare-and-swap and a replay record. The
    /// transaction-local admission setting is the database-side no-silent-
    /// activation guard.
    #[allow(clippy::too_many_arguments)]
    pub async fn approve_memory_proposal(
        &self,
        tenant_id: TenantId,
        proposal_id: Uuid,
        reviewer_id: Uuid,
        expected_version: i64,
        idempotency_key: &str,
        memory_id: Uuid,
        memory_version_id: Uuid,
        claim: &str,
        scope: &str,
        applies_when: &str,
        reason: &str,
        evidence: &serde_json::Value,
    ) -> Result<Uuid, ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        if let Some(row) = sqlx::query("SELECT response FROM memory_review_operations WHERE tenant_id = $1 AND proposal_id = $2 AND idempotency_key = $3")
            .bind(tenant_id.as_uuid()).bind(proposal_id).bind(idempotency_key).fetch_optional(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })? {
            return Uuid::parse_str(
                row.get::<serde_json::Value, _>("response")
                    .get("memory_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
            )
            .map_err(|_| ApplicationError::InternalUnexpected);
        }
        let changed = sqlx::query("UPDATE proposals SET state = 'approved', reviewed_by = $4, reviewed_at = now(), version = version + 1 WHERE tenant_id = $1 AND proposal_id = $2 AND version = $3 AND state = 'pending'")
            .bind(tenant_id.as_uuid()).bind(proposal_id).bind(expected_version).bind(reviewer_id).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres"})?;
        if changed.rows_affected() == 0 {
            return Err(ApplicationError::VersionConflict {
                resource: "proposal",
                current_version: (expected_version + 1).max(0) as u64,
            });
        }
        sqlx::query("SET LOCAL app.memory_admission = 'approved'")
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        let hash = format!(
            "{:x}",
            Sha256::digest(claim.trim().to_lowercase().as_bytes())
        );
        sqlx::query("INSERT INTO memories (memory_id, tenant_id, area_id, state, origin, current_version_id, version) SELECT $1, tenant_id, area_id, 'active', 'approved', NULL, 1 FROM proposals WHERE tenant_id = $2 AND proposal_id = $3")
            .bind(memory_id).bind(tenant_id.as_uuid()).bind(proposal_id).execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        sqlx::query("INSERT INTO memory_versions (memory_version_id, tenant_id, memory_id, version_number, state, claim, scope, applies_when, reason, evidence, claim_hash, owner_actor_id) VALUES ($1, $2, $3, 1, 'current', $4, $5, $6, $7, $8, $9, CASE WHEN $5 LIKE 'personal%' OR $5 LIKE 'private%' THEN $10 ELSE NULL END)")
            .bind(memory_version_id).bind(tenant_id.as_uuid()).bind(memory_id).bind(claim).bind(scope).bind(applies_when).bind(reason).bind(evidence).bind(&hash).bind(reviewer_id).execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        sqlx::query(
            "UPDATE memories SET current_version_id = $3 WHERE tenant_id = $1 AND memory_id = $2",
        )
        .bind(tenant_id.as_uuid())
        .bind(memory_id)
        .bind(memory_version_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        let response = serde_json::json!({"memory_id": memory_id.to_string(), "proposal_id": proposal_id.to_string()});
        sqlx::query("INSERT INTO memory_review_operations (tenant_id, proposal_id, idempotency_key, request_hash, response, created_at) VALUES ($1, $2, $3, $4, $5, now())")
            .bind(tenant_id.as_uuid()).bind(proposal_id).bind(idempotency_key).bind(hash).bind(&response).execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(memory_id)
    }

    pub async fn reject_memory_proposal(
        &self,
        tenant_id: TenantId,
        proposal_id: Uuid,
        reviewer_id: Uuid,
        expected_version: i64,
        idempotency_key: &str,
        reason: &str,
    ) -> Result<(), ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        if sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM memory_review_operations WHERE tenant_id=$1 AND proposal_id=$2 AND idempotency_key=$3)").bind(tenant_id.as_uuid()).bind(proposal_id).bind(idempotency_key).fetch_one(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })? { return Ok(()); }
        let changed = sqlx::query("UPDATE proposals SET state='rejected', rejection_reason=$4, version=version+1 WHERE tenant_id=$1 AND proposal_id=$2 AND version=$3 AND state='pending'").bind(tenant_id.as_uuid()).bind(proposal_id).bind(expected_version).bind(reason).execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if changed.rows_affected() == 0 {
            return Err(ApplicationError::VersionConflict {
                resource: "proposal",
                current_version: (expected_version + 1).max(0) as u64,
            });
        }
        sqlx::query("INSERT INTO memory_review_operations (tenant_id,proposal_id,idempotency_key,request_hash,response,created_at) VALUES ($1,$2,$3,$4,$5,now())").bind(tenant_id.as_uuid()).bind(proposal_id).bind(idempotency_key).bind(format!("reject:{reviewer_id}:{reason}")).bind(serde_json::json!({"state":"rejected","reviewer_id":reviewer_id})).execute(&mut *tx).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })
    }

    /// Creates a Map draft. Mirrors `create_memory_proposal`: a plain
    /// insert, `ON CONFLICT DO NOTHING` for idempotent creation-by-id.
    pub async fn create_map_draft(
        &self,
        tenant_id: TenantId,
        map_version_id: Uuid,
        area_id: Uuid,
        version_number: i64,
        definition: &serde_json::Value,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO map_versions (map_version_id, tenant_id, area_id, version_number, state, definition, version) VALUES ($1, $2, $3, $4, 'draft', $5, 1) ON CONFLICT (map_version_id) DO NOTHING")
            .bind(map_version_id)
            .bind(tenant_id.as_uuid())
            .bind(area_id)
            .bind(version_number)
            .bind(definition)
            .execute(&self.pool)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    /// Publishes a Map draft with compare-and-swap and a replay record,
    /// mirroring `approve_memory_proposal`. Also updates the owning Area's
    /// `current_map_version_id` in the same transaction. The transaction-
    /// local `app.map_publication_admission` setting is the database-side
    /// no-silent-publication guard (`require_map_publication_admission()`,
    /// `migrations/20260808100000_phase_f_live_adapter.sql`).
    pub async fn publish_map_draft(
        &self,
        tenant_id: TenantId,
        map_version_id: Uuid,
        expected_version: i64,
        idempotency_key: &str,
    ) -> Result<Uuid, ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        if let Some(row) = sqlx::query("SELECT response FROM map_review_operations WHERE tenant_id = $1 AND map_version_id = $2 AND idempotency_key = $3")
            .bind(tenant_id.as_uuid()).bind(map_version_id).bind(idempotency_key).fetch_optional(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })? {
            return Uuid::parse_str(
                row.get::<serde_json::Value, _>("response")
                    .get("map_version_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
            )
            .map_err(|_| ApplicationError::InternalUnexpected);
        }
        // The CAS step only bumps `version`, leaving `state` untouched, so
        // the publication-admission trigger (which only fires when
        // `NEW.state = 'published'`) does not see a gated transition yet —
        // admission is set immediately below, before the actual state flip.
        let changed = sqlx::query("UPDATE map_versions SET version = version + 1 WHERE tenant_id = $1 AND map_version_id = $2 AND version = $3 AND state = 'draft'")
            .bind(tenant_id.as_uuid()).bind(map_version_id).bind(expected_version).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if changed.rows_affected() == 0 {
            return Err(ApplicationError::VersionConflict {
                resource: "map_version",
                current_version: (expected_version + 1).max(0) as u64,
            });
        }
        sqlx::query("SET LOCAL app.map_publication_admission = 'approved'")
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        sqlx::query("UPDATE map_versions SET state = 'published' WHERE tenant_id = $1 AND map_version_id = $2")
            .bind(tenant_id.as_uuid())
            .bind(map_version_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        sqlx::query("UPDATE areas SET current_map_version_id = $2 WHERE tenant_id = $1 AND area_id = (SELECT area_id FROM map_versions WHERE map_version_id = $2)")
            .bind(tenant_id.as_uuid())
            .bind(map_version_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        let response = serde_json::json!({"map_version_id": map_version_id.to_string()});
        sqlx::query("INSERT INTO map_review_operations (tenant_id, map_version_id, idempotency_key, response, created_at) VALUES ($1, $2, $3, $4, now())")
            .bind(tenant_id.as_uuid()).bind(map_version_id).bind(idempotency_key).bind(&response).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(map_version_id)
    }

    /// Creates an Area-local Entity proposal (`state = 'proposed'`).
    #[allow(clippy::too_many_arguments)]
    pub async fn create_entity(
        &self,
        tenant_id: TenantId,
        entity_id: Uuid,
        area_id: Uuid,
        map_version_id: Uuid,
        kind: &str,
        origin: &str,
        descriptor: &serde_json::Value,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO entities (entity_id, tenant_id, area_id, map_version_id, state, kind, origin, descriptor, version) VALUES ($1, $2, $3, $4, 'proposed', $5, $6, $7, 1) ON CONFLICT (entity_id) DO NOTHING")
            .bind(entity_id)
            .bind(tenant_id.as_uuid())
            .bind(area_id)
            .bind(map_version_id)
            .bind(kind)
            .bind(origin)
            .bind(descriptor)
            .execute(&self.pool)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    /// Approves a proposed Entity (`state = 'active'`) with compare-and-swap
    /// and a replay record. `app.entity_admission` is the database-side
    /// no-silent-activation guard shared by `entities` and `relationships`
    /// (`require_entity_admission()`/`require_relationship_admission()`).
    pub async fn approve_entity(
        &self,
        tenant_id: TenantId,
        entity_id: Uuid,
        expected_version: i64,
        idempotency_key: &str,
    ) -> Result<Uuid, ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        if let Some(row) = sqlx::query("SELECT response FROM entity_review_operations WHERE tenant_id = $1 AND entity_id = $2 AND idempotency_key = $3")
            .bind(tenant_id.as_uuid()).bind(entity_id).bind(idempotency_key).fetch_optional(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })? {
            return Uuid::parse_str(
                row.get::<serde_json::Value, _>("response")
                    .get("entity_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
            )
            .map_err(|_| ApplicationError::InternalUnexpected);
        }
        // The CAS step only bumps `version`, leaving `state` untouched, so
        // the entity-admission trigger (fires only when `NEW.state =
        // 'active'`) does not see a gated transition yet — admission is set
        // immediately below, before the actual state flip.
        let changed = sqlx::query("UPDATE entities SET version = version + 1 WHERE tenant_id = $1 AND entity_id = $2 AND version = $3 AND state = 'proposed'")
            .bind(tenant_id.as_uuid()).bind(entity_id).bind(expected_version).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if changed.rows_affected() == 0 {
            return Err(ApplicationError::VersionConflict {
                resource: "entity",
                current_version: (expected_version + 1).max(0) as u64,
            });
        }
        sqlx::query("SET LOCAL app.entity_admission = 'approved'")
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        sqlx::query("UPDATE entities SET state = 'active' WHERE tenant_id = $1 AND entity_id = $2")
            .bind(tenant_id.as_uuid())
            .bind(entity_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        let response = serde_json::json!({"entity_id": entity_id.to_string()});
        sqlx::query("INSERT INTO entity_review_operations (tenant_id, entity_id, idempotency_key, response, created_at) VALUES ($1, $2, $3, $4, now())")
            .bind(tenant_id.as_uuid()).bind(entity_id).bind(idempotency_key).bind(&response).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(entity_id)
    }

    /// Creates an Area-local Relationship proposal (`state = 'proposed'`).
    #[allow(clippy::too_many_arguments)]
    pub async fn create_relationship(
        &self,
        tenant_id: TenantId,
        relationship_id: Uuid,
        area_id: Uuid,
        map_version_id: Uuid,
        source_entity_id: Uuid,
        target_entity_id: Uuid,
        relation_kind: &str,
        origin: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO relationships (relationship_id, tenant_id, area_id, map_version_id, source_entity_id, target_entity_id, relation_kind, state, origin, version) VALUES ($1, $2, $3, $4, $5, $6, $7, 'proposed', $8, 1) ON CONFLICT (relationship_id) DO NOTHING")
            .bind(relationship_id)
            .bind(tenant_id.as_uuid())
            .bind(area_id)
            .bind(map_version_id)
            .bind(source_entity_id)
            .bind(target_entity_id)
            .bind(relation_kind)
            .bind(origin)
            .execute(&self.pool)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    /// Approves a proposed Relationship (`state = 'active'`) with compare-
    /// and-swap and a replay record.
    pub async fn approve_relationship(
        &self,
        tenant_id: TenantId,
        relationship_id: Uuid,
        expected_version: i64,
        idempotency_key: &str,
    ) -> Result<Uuid, ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        if let Some(row) = sqlx::query("SELECT response FROM relationship_review_operations WHERE tenant_id = $1 AND relationship_id = $2 AND idempotency_key = $3")
            .bind(tenant_id.as_uuid()).bind(relationship_id).bind(idempotency_key).fetch_optional(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })? {
            return Uuid::parse_str(
                row.get::<serde_json::Value, _>("response")
                    .get("relationship_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
            )
            .map_err(|_| ApplicationError::InternalUnexpected);
        }
        // Same two-step CAS-then-admission-then-state-flip shape as
        // `approve_entity` above, for the same reason (the relationship-
        // admission trigger fires only when `NEW.state = 'active'`).
        let changed = sqlx::query("UPDATE relationships SET version = version + 1 WHERE tenant_id = $1 AND relationship_id = $2 AND version = $3 AND state = 'proposed'")
            .bind(tenant_id.as_uuid()).bind(relationship_id).bind(expected_version).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if changed.rows_affected() == 0 {
            return Err(ApplicationError::VersionConflict {
                resource: "relationship",
                current_version: (expected_version + 1).max(0) as u64,
            });
        }
        sqlx::query("SET LOCAL app.entity_admission = 'approved'")
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        sqlx::query("UPDATE relationships SET state = 'active' WHERE tenant_id = $1 AND relationship_id = $2")
            .bind(tenant_id.as_uuid())
            .bind(relationship_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        let response = serde_json::json!({"relationship_id": relationship_id.to_string()});
        sqlx::query("INSERT INTO relationship_review_operations (tenant_id, relationship_id, idempotency_key, response, created_at) VALUES ($1, $2, $3, $4, now())")
            .bind(tenant_id.as_uuid()).bind(relationship_id).bind(idempotency_key).bind(&response).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(relationship_id)
    }

    /// Creates a Cross-Map mapping proposal (`state = 'proposed'`).
    #[allow(clippy::too_many_arguments)]
    pub async fn create_cross_map_mapping(
        &self,
        tenant_id: TenantId,
        cross_map_mapping_id: Uuid,
        source_area_id: Uuid,
        target_area_id: Uuid,
        source_map_version_id: Uuid,
        target_map_version_id: Uuid,
        relation: &str,
        rationale: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO cross_map_mappings (cross_map_mapping_id, tenant_id, source_area_id, target_area_id, source_map_version_id, target_map_version_id, relation, state, rationale, version) VALUES ($1, $2, $3, $4, $5, $6, $7, 'proposed', $8, 1) ON CONFLICT (cross_map_mapping_id) DO NOTHING")
            .bind(cross_map_mapping_id)
            .bind(tenant_id.as_uuid())
            .bind(source_area_id)
            .bind(target_area_id)
            .bind(source_map_version_id)
            .bind(target_map_version_id)
            .bind(relation)
            .bind(rationale)
            .execute(&self.pool)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    /// Approves a proposed Cross-Map mapping (`state = 'approved'`) with
    /// compare-and-swap and a replay record. `app.cross_map_admission` is
    /// the database-side no-silent-activation guard
    /// (`require_cross_map_admission()`) — the same structural defense in
    /// depth Phase F's core `is_traversable` predicate relies on at the
    /// application layer.
    pub async fn approve_cross_map_mapping(
        &self,
        tenant_id: TenantId,
        cross_map_mapping_id: Uuid,
        expected_version: i64,
        idempotency_key: &str,
    ) -> Result<Uuid, ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        if let Some(row) = sqlx::query("SELECT response FROM cross_map_review_operations WHERE tenant_id = $1 AND cross_map_mapping_id = $2 AND idempotency_key = $3")
            .bind(tenant_id.as_uuid()).bind(cross_map_mapping_id).bind(idempotency_key).fetch_optional(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })? {
            return Uuid::parse_str(
                row.get::<serde_json::Value, _>("response")
                    .get("cross_map_mapping_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
            )
            .map_err(|_| ApplicationError::InternalUnexpected);
        }
        // Same two-step CAS-then-admission-then-state-flip shape as
        // `publish_map_draft`/`approve_entity` above (the cross-map-
        // admission trigger fires only when `NEW.state = 'approved'`).
        let changed = sqlx::query("UPDATE cross_map_mappings SET version = version + 1 WHERE tenant_id = $1 AND cross_map_mapping_id = $2 AND version = $3 AND state = 'proposed'")
            .bind(tenant_id.as_uuid()).bind(cross_map_mapping_id).bind(expected_version).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if changed.rows_affected() == 0 {
            return Err(ApplicationError::VersionConflict {
                resource: "cross_map_mapping",
                current_version: (expected_version + 1).max(0) as u64,
            });
        }
        sqlx::query("SET LOCAL app.cross_map_admission = 'approved'")
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        sqlx::query("UPDATE cross_map_mappings SET state = 'approved' WHERE tenant_id = $1 AND cross_map_mapping_id = $2")
            .bind(tenant_id.as_uuid())
            .bind(cross_map_mapping_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        let response =
            serde_json::json!({"cross_map_mapping_id": cross_map_mapping_id.to_string()});
        sqlx::query("INSERT INTO cross_map_review_operations (tenant_id, cross_map_mapping_id, idempotency_key, response, created_at) VALUES ($1, $2, $3, $4, now())")
            .bind(tenant_id.as_uuid()).bind(cross_map_mapping_id).bind(idempotency_key).bind(&response).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(cross_map_mapping_id)
    }

    /// Atomically claims queued work or work whose lease expired. The state
    /// predicate and token update happen in one statement, so two workers
    /// cannot receive the same lease.
    pub async fn claim_operation(
        &self,
        tenant_id: TenantId,
        lease_token: &str,
        lease_seconds: i64,
    ) -> Result<Option<DurableLease>, ApplicationError> {
        let row = sqlx::query(
            "UPDATE operations SET state = 'running', attempt = attempt + 1,
                 lease_token = $2, lease_expires_at = now() + make_interval(secs => $3),
                 updated_at = now()
             WHERE operation_id = (
                 SELECT operation_id FROM operations
                 WHERE tenant_id = $1 AND cancel_requested = false
                   AND (state = 'queued' OR (state IN ('leased', 'running') AND lease_expires_at <= now()))
                   AND attempt < max_attempts
                 ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1
             )
             RETURNING operation_id, attempt, checkpoint, payload",
        )
        .bind(tenant_id.as_uuid())
        .bind(lease_token)
        .bind(lease_seconds.max(1))
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(row.map(|row| DurableLease {
            operation_id: OperationId::from(row.get::<Uuid, _>("operation_id")),
            lease_token: lease_token.to_owned(),
            attempt: row.get::<i32, _>("attempt") as u32,
            checkpoint: row.try_get("checkpoint").ok(),
            payload: row.get("payload"),
        }))
    }

    pub async fn enqueue_operation(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        payload: &serde_json::Value,
        scope: &str,
        key: &str,
        max_attempts: i32,
    ) -> Result<OperationId, ApplicationError> {
        let row = sqlx::query(
            "INSERT INTO operations (operation_id, tenant_id, state, max_attempts, payload, idempotency_scope, idempotency_key, created_at, updated_at)
             VALUES ($1, $2, 'queued', $6, $3, $4, $5, now(), now())
             ON CONFLICT (tenant_id, idempotency_scope, idempotency_key)
             WHERE idempotency_scope IS NOT NULL AND idempotency_key IS NOT NULL
             DO UPDATE SET operation_id = operations.operation_id
             RETURNING operation_id",
        )
        .bind(operation_id.as_uuid()).bind(tenant_id.as_uuid()).bind(payload)
        .bind(scope).bind(key).bind(max_attempts.max(1))
        .fetch_one(&self.pool).await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(OperationId::from(row.get::<Uuid, _>("operation_id")))
    }

    pub async fn renew_operation(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        lease_token: &str,
        lease_seconds: i64,
    ) -> Result<(), ApplicationError> {
        let result = sqlx::query(
            "UPDATE operations SET lease_expires_at = now() + make_interval(secs => $4), updated_at = now() WHERE tenant_id = $1 AND operation_id = $2 AND lease_token = $3 AND state = 'running'",
        )
        .bind(tenant_id.as_uuid())
        .bind(operation_id.as_uuid())
        .bind(lease_token)
        .bind(lease_seconds.max(1))
        .execute(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        Ok(())
    }

    pub async fn update_source_state(
        &self,
        tenant_id: TenantId,
        source_id: Uuid,
        source_version_id: Option<Uuid>,
        state: &str,
        quarantine_reason: Option<&str>,
    ) -> Result<(), ApplicationError> {
        let result = sqlx::query(
            "UPDATE sources SET state = $3, updated_at = now(), version = version + 1
             WHERE tenant_id = $1 AND source_id = $2",
        )
        .bind(tenant_id.as_uuid())
        .bind(source_id)
        .bind(state)
        .execute(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::ResourceNotFound { resource: "source" });
        }
        if let (Some(version_id), Some(reason)) = (source_version_id, quarantine_reason) {
            sqlx::query("UPDATE source_versions SET state = 'quarantined', quarantine_reason = $3 WHERE tenant_id = $1 AND source_version_id = $2")
                .bind(tenant_id.as_uuid()).bind(version_id).bind(reason).execute(&self.pool).await
                .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn persist_text_output(
        &self,
        tenant_id: TenantId,
        source_version_id: Uuid,
        artifact_id: Uuid,
        processor: &str,
        processor_version: &str,
        input_hash: &str,
        chunks: &[(Uuid, &str, &str, &str, &str)],
        warnings: &[String],
    ) -> Result<(), ApplicationError> {
        let mut tx =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        sqlx::query("INSERT INTO artifacts (artifact_id, tenant_id, source_version_id, processor_name, processor_version, input_hash, state, descriptor, warnings) VALUES ($1, $2, $3, $4, $5, $6, 'available', $7, $8) ON CONFLICT (source_version_id, processor_name, processor_version, input_hash) DO NOTHING")
            .bind(artifact_id).bind(tenant_id.as_uuid()).bind(source_version_id).bind(processor).bind(processor_version).bind(input_hash)
            .bind(serde_json::json!({"kind":"text"})).bind(serde_json::json!(warnings)).execute(&mut *tx).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        for (chunk_id, representation, coordinate, content_hash, content) in chunks {
            sqlx::query("INSERT INTO chunks (chunk_id, tenant_id, source_version_id, artifact_id, representation, coordinate, content_hash, content, state) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'created') ON CONFLICT (source_version_id, representation, coordinate, content_hash) DO NOTHING")
                .bind(chunk_id).bind(tenant_id.as_uuid()).bind(source_version_id).bind(artifact_id).bind(representation).bind(coordinate).bind(content_hash).bind(content).execute(&mut *tx).await
                .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        }
        tx.commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start_processor_run(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        source_version_id: Uuid,
        processor_run_id: Uuid,
        processor_name: &str,
        processor_version: &str,
        configuration_fingerprint: &str,
        input_hash: &str,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO processor_runs (processor_run_id, tenant_id, operation_id, source_version_id, processor_name, processor_version, configuration_fingerprint, input_hash, state, started_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'running', now()) ON CONFLICT (processor_run_id) DO NOTHING")
            .bind(processor_run_id).bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).bind(source_version_id).bind(processor_name).bind(processor_version).bind(configuration_fingerprint).bind(input_hash).execute(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(())
    }

    pub async fn finish_processor_run(
        &self,
        tenant_id: TenantId,
        processor_run_id: Uuid,
        state: &str,
        warnings: &[String],
    ) -> Result<(), ApplicationError> {
        sqlx::query("UPDATE processor_runs SET state = $3, warnings = $4, finished_at = now() WHERE tenant_id = $1 AND processor_run_id = $2")
            .bind(tenant_id.as_uuid()).bind(processor_run_id).bind(state).bind(serde_json::json!(warnings)).execute(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(())
    }

    pub async fn append_process_evidence(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        evidence_id: Uuid,
        kind: &str,
        payload: &serde_json::Value,
    ) -> Result<(), ApplicationError> {
        sqlx::query("INSERT INTO process_evidence (process_evidence_id, tenant_id, operation_id, kind, payload, created_at) VALUES ($1, $2, $3, $4, $5, now()) ON CONFLICT (process_evidence_id) DO NOTHING")
            .bind(evidence_id).bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).bind(kind).bind(payload).execute(&self.pool).await
            .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        Ok(())
    }

    pub async fn save_checkpoint(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        lease_token: &str,
        key: &str,
        payload: &serde_json::Value,
        progress: i16,
    ) -> Result<(), ApplicationError> {
        let mut transaction =
            self.pool
                .begin()
                .await
                .map_err(|_| ApplicationError::DependencyUnavailable {
                    dependency: "postgres",
                })?;
        let result = sqlx::query(
            "UPDATE operations
             SET progress = $4,
                 checkpoint = jsonb_build_object('key', $5::text, 'payload', $6::jsonb),
                 updated_at = now()
             WHERE operation_id = $1 AND tenant_id = $2 AND lease_token = $3 AND state = 'running'",
        )
        .bind(operation_id.as_uuid())
        .bind(tenant_id.as_uuid())
        .bind(lease_token)
        .bind(progress.clamp(0, 100))
        .bind(key)
        .bind(payload)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        sqlx::query(
            "INSERT INTO operation_checkpoints (operation_id, tenant_id, checkpoint_key, payload, progress, created_at)
             VALUES ($1, $2, $3, $4, $5, now())
             ON CONFLICT (operation_id, checkpoint_key) DO UPDATE
             SET payload = EXCLUDED.payload, progress = EXCLUDED.progress, created_at = EXCLUDED.created_at",
        )
        .bind(operation_id.as_uuid())
        .bind(tenant_id.as_uuid())
        .bind(key)
        .bind(payload)
        .bind(progress.clamp(0, 100))
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        transaction
            .commit()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "postgres",
            })?;
        Ok(())
    }

    pub async fn request_operation_cancel(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
    ) -> Result<(), ApplicationError> {
        let result = sqlx::query(
            "UPDATE operations SET cancel_requested = true,
                 state = CASE WHEN state = 'queued' THEN 'cancelled' ELSE state END, updated_at = now()
             WHERE tenant_id = $1 AND operation_id = $2 AND state NOT IN ('succeeded', 'failed', 'cancelled')",
        ).bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).execute(&self.pool).await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        Ok(())
    }

    pub async fn is_cancel_requested(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
    ) -> Result<bool, ApplicationError> {
        sqlx::query(
            "SELECT cancel_requested FROM operations WHERE tenant_id = $1 AND operation_id = $2",
        )
        .bind(tenant_id.as_uuid())
        .bind(operation_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?
        .map(|row| row.get("cancel_requested"))
        .ok_or(ApplicationError::OperationNotFound)
    }

    pub async fn fail_operation(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        lease_token: &str,
        error_code: &str,
        error_message: &str,
    ) -> Result<(), ApplicationError> {
        let result = sqlx::query("UPDATE operations SET state = CASE WHEN cancel_requested THEN 'cancelled' WHEN attempt < max_attempts THEN 'queued' ELSE 'failed' END, error_code = $4, error_message = $5, lease_token = NULL, lease_expires_at = NULL, updated_at = now() WHERE tenant_id = $1 AND operation_id = $2 AND lease_token = $3")
            .bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).bind(lease_token).bind(error_code).bind(error_message)
            .execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        Ok(())
    }

    pub async fn manual_retry_operation(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
    ) -> Result<(), ApplicationError> {
        let result = sqlx::query(
            "UPDATE operations SET state = 'queued', attempt = 0, cancel_requested = false, lease_token = NULL, lease_expires_at = NULL, error_code = NULL, error_message = NULL, updated_at = now() WHERE tenant_id = $1 AND operation_id = $2 AND state = 'failed'",
        )
        .bind(tenant_id.as_uuid())
        .bind(operation_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        Ok(())
    }

    pub async fn finish_operation(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        lease_token: &str,
        state: OperationState,
        error: Option<&str>,
    ) -> Result<(), ApplicationError> {
        let state = match state {
            OperationState::Succeeded => "succeeded",
            OperationState::Failed => "failed",
            OperationState::Cancelled => "cancelled",
            _ => {
                return Err(ApplicationError::InvalidRequest {
                    message: "operation cannot finish in this state".into(),
                })
            }
        };
        let result = sqlx::query("UPDATE operations SET state = $4, lease_token = NULL, lease_expires_at = NULL, progress = CASE WHEN $4 = 'succeeded' THEN 100 ELSE progress END, error_message = $5, updated_at = now() WHERE tenant_id = $1 AND operation_id = $2 AND lease_token = $3")
            .bind(tenant_id.as_uuid()).bind(operation_id.as_uuid()).bind(lease_token).bind(state).bind(error)
            .execute(&self.pool).await.map_err(|_| ApplicationError::DependencyUnavailable { dependency: "postgres" })?;
        if result.rows_affected() == 0 {
            return Err(ApplicationError::OperationNotFound);
        }
        Ok(())
    }
}

fn parse_entity_state(value: &str) -> EntityState {
    match value {
        "proposed" => EntityState::Proposed,
        "merged" => EntityState::Merged,
        "retired" => EntityState::Retired,
        _ => EntityState::Active,
    }
}

fn parse_relationship_state(value: &str) -> RelationshipState {
    match value {
        "proposed" => RelationshipState::Proposed,
        "superseded" => RelationshipState::Superseded,
        "rejected" => RelationshipState::Rejected,
        "retired" => RelationshipState::Retired,
        _ => RelationshipState::Active,
    }
}

fn parse_origin(value: &str) -> Origin {
    match value {
        "observed" => Origin::Observed,
        "inferred" => Origin::Inferred,
        "approved" => Origin::Approved,
        _ => Origin::Proposed,
    }
}

fn workspace_interview_json(row: &sqlx::postgres::PgRow) -> serde_json::Value {
    serde_json::json!({
        "interview_id": row.get::<Uuid, _>("interview_id"),
        "tenant_id": row.get::<Uuid, _>("tenant_id"),
        "state": row.get::<String, _>("state"),
        "selected_area_ids": row.get::<serde_json::Value, _>("selected_area_ids"),
        "requested_areas": row.get::<serde_json::Value, _>("requested_areas"),
        "ontology_draft": row.get::<serde_json::Value, _>("ontology_draft"),
        "confirmed": row.get::<bool, _>("confirmed"),
        "proposal_id": row.try_get::<Uuid, _>("proposal_id").ok(),
        "version": row.get::<i64, _>("version"),
        "content": format!("# Workspace setup\nState: {}\nInterview: {}\nSubmission creates a proposal only; Area access and Map publication remain separately governed.", row.get::<String, _>("state"), row.get::<Uuid, _>("interview_id")),
    })
}

#[async_trait]
impl Repository for PgRepository {
    async fn append_event(&self, event: &DomainEvent) -> Result<(), ApplicationError> {
        sqlx::query(
            "INSERT INTO events
             (event_id, tenant_id, actor_id, agent_identity_id, session_id, action,
              target_type, target_id, previous_version, resulting_version, reason,
              policy_result, request_id, idempotency_key, occurred_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(event.event_id.as_uuid())
        .bind(event.tenant_id.as_uuid())
        .bind(event.actor_id)
        .bind(event.agent_identity_id.map(Into::<Uuid>::into))
        .bind(event.session_id)
        .bind(&event.action)
        .bind(&event.target_type)
        .bind(&event.target_id)
        .bind(event.previous_version.as_ref().map(|v| v.0 as i64))
        .bind(event.resulting_version.as_ref().map(|v| v.0 as i64))
        .bind(&event.reason)
        .bind(event.policy_result.as_ref().map(|policy| {
            serde_json::json!({
                "effect": format!("{:?}", policy.effect).to_lowercase(),
                "rule_id": policy.rule_id.as_uuid(),
                "rule_version": policy.rule_version.as_uuid(),
                "rationale": policy.rationale,
            })
        }))
        .bind(&event.request_id)
        .bind(&event.idempotency_key)
        .bind(event.occurred_at)
        .execute(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        Ok(())
    }

    async fn claim_idempotency(&self, key: &IdempotencyKey) -> Result<(), ApplicationError> {
        sqlx::query(
            "INSERT INTO idempotency_keys (tenant_id, scope, key, request_hash, created_at)
             VALUES ($1, $2, $3, $4, now()) ON CONFLICT (tenant_id, scope, key) DO NOTHING",
        )
        .bind(key.tenant_id.as_uuid())
        .bind(&key.scope)
        .bind(&key.value)
        .bind(
            key.principal_id
                .map(Into::<Uuid>::into)
                .map(|id| id.to_string()),
        )
        .execute(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        Ok(())
    }

    async fn current_version(
        &self,
        tenant_id: TenantId,
        resource_id: Uuid,
    ) -> Result<VersionToken, ApplicationError> {
        let row = sqlx::query(
            "SELECT version FROM entities WHERE tenant_id = $1 AND entity_id = $2
             UNION ALL SELECT version FROM memories WHERE tenant_id = $1 AND memory_id = $2
             UNION ALL SELECT version FROM proposals WHERE tenant_id = $1 AND proposal_id = $2
             LIMIT 1",
        )
        .bind(tenant_id.as_uuid())
        .bind(resource_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ApplicationError::DependencyUnavailable {
            dependency: "postgres",
        })?;
        row.map(|row| VersionToken(row.get::<i64, _>("version") as u64))
            .ok_or(ApplicationError::ResourceNotFound {
                resource: "versioned_resource",
            })
    }
}

/// Stable, tenant-prefixed object key. The source filename is deliberately
/// excluded so renames cannot create a second logical object.
pub fn source_object_key(tenant_id: TenantId, source_id: Uuid, source_version_id: Uuid) -> String {
    format!(
        "tenants/{}/sources/{}/versions/{}",
        tenant_id.as_uuid(),
        source_id,
        source_version_id
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizationMetadata {
    pub tenant_id: TenantId,
    pub object_key: String,
    pub expected_key: String,
    pub byte_size: u64,
    pub expected_byte_size: u64,
    pub media_type: String,
    pub expected_media_type: String,
    pub checksum: String,
    pub expected_checksum: String,
}

pub fn validate_finalization(metadata: &FinalizationMetadata) -> Result<(), ApplicationError> {
    let expected_prefix = format!("tenants/{}/", metadata.tenant_id.as_uuid());
    if !metadata.object_key.starts_with(&expected_prefix)
        || metadata.object_key != metadata.expected_key
        || metadata.byte_size != metadata.expected_byte_size
        || metadata.media_type != metadata.expected_media_type
        || metadata.checksum != metadata.expected_checksum
    {
        return Err(ApplicationError::InvalidRequest {
            message: "staged object metadata does not match the source version".to_owned(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct S3ObjectStore {
    client: S3Client,
    bucket: String,
}

impl S3ObjectStore {
    pub fn new(client: S3Client, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    pub async fn ensure_bucket(&self) -> Result<(), ApplicationError> {
        if self
            .client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .is_ok()
        {
            return Ok(());
        }
        self.client
            .create_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "object_store",
            })?;
        Ok(())
    }
}

#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put(
        &self,
        tenant_id: TenantId,
        object_key: &str,
        bytes: &[u8],
    ) -> Result<(), ApplicationError> {
        if !object_key.starts_with(&format!("tenants/{}/", tenant_id.as_uuid())) {
            return Err(ApplicationError::Forbidden);
        }
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(object_key)
            .body(ByteStream::from(bytes.to_vec()))
            .send()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "object_store",
            })?;
        Ok(())
    }

    async fn delete(&self, tenant_id: TenantId, object_key: &str) -> Result<(), ApplicationError> {
        if !object_key.starts_with(&format!("tenants/{}/", tenant_id.as_uuid())) {
            return Err(ApplicationError::Forbidden);
        }
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(object_key)
            .send()
            .await
            .map_err(|_| ApplicationError::DependencyUnavailable {
                dependency: "object_store",
            })?;
        Ok(())
    }
}

/// Compatibility marker retained from the Phase A scaffold.
pub fn storage_crate_placeholder() -> &'static str {
    "engrave-contracts"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_keys_are_stable_and_tenant_scoped() {
        let tenant = TenantId::new(Uuid::nil());
        assert_eq!(
            source_object_key(tenant, Uuid::from_u128(1), Uuid::from_u128(2)),
            "tenants/00000000-0000-0000-0000-000000000000/sources/00000000-0000-0000-0000-000000000001/versions/00000000-0000-0000-0000-000000000002"
        );
    }

    #[test]
    fn finalization_rejects_mismatched_metadata() {
        let tenant = TenantId::new(Uuid::nil());
        let metadata = FinalizationMetadata {
            tenant_id: tenant,
            object_key: "tenants/00000000-0000-0000-0000-000000000000/a".into(),
            expected_key: "tenants/00000000-0000-0000-0000-000000000000/b".into(),
            byte_size: 1,
            expected_byte_size: 2,
            media_type: "text/plain".into(),
            expected_media_type: "application/pdf".into(),
            checksum: "a".into(),
            expected_checksum: "b".into(),
        };
        assert!(validate_finalization(&metadata).is_err());
    }

    #[test]
    fn links_against_core() {
        assert_eq!(storage_crate_placeholder(), "engrave-contracts");
    }

    #[test]
    fn provider_partial_batch_is_rejected_without_partial_progress() {
        let result = validate_embedding_batch(
            vec![VoyageEmbedding {
                embedding: vec![0.0; 512],
            }],
            2,
            512,
        );
        assert!(matches!(
            result,
            Err(ProviderError::InvalidRequest(message)) if message.contains("partial batch")
        ));
    }

    #[test]
    fn provider_batch_dimension_mismatch_is_rejected() {
        let result = validate_embedding_batch(
            vec![VoyageEmbedding {
                embedding: vec![0.0; 511],
            }],
            1,
            512,
        );
        assert!(matches!(
            result,
            Err(ProviderError::DimensionMismatch {
                expected: 512,
                actual: 511
            })
        ));
    }

    #[tokio::test]
    async fn transient_provider_failure_is_retried_but_authentication_is_not() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock provider");
        let endpoint = format!(
            "http://{}/v1/embeddings",
            listener.local_addr().expect("mock address")
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let observed_calls = Arc::clone(&calls);
        let server = std::thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().expect("accept provider request");
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                let call = observed_calls.fetch_add(1, Ordering::SeqCst);
                let (status, body) = if call == 0 {
                    ("503 Service Unavailable", "{}".to_owned())
                } else if call == 1 {
                    let vector = vec![0.0_f32; 512];
                    (
                        "200 OK",
                        serde_json::json!({"data":[{"embedding":vector}]}).to_string(),
                    )
                } else {
                    ("401 Unauthorized", "{}".to_owned())
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .expect("write provider response");
            }
        });
        let client = VoyageEmbeddingClient {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .expect("client"),
            endpoint,
            model: "test".into(),
            api_key: "test-secret-never-logged".into(),
            native_dimension: 512,
        };

        let values = client.embed("retry me").await.expect("503 should retry");
        assert_eq!(values.len(), 512);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let error = client
            .embed("do not retry auth")
            .await
            .expect_err("401 must fail");
        assert!(matches!(error, ProviderError::Authentication));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        server.join().expect("mock provider thread");
    }
}
