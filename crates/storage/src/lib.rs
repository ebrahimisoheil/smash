//! SQLx/PostgreSQL and S3-compatible object-store adapters.
//!
//! The adapters keep framework concerns out of the storage boundary. SQLx
//! queries are runtime queries until the committed offline cache is generated
//! against the canonical migration; the migration itself remains the source
//! of truth for schema shape.
#![forbid(unsafe_code)]

use arrow_array::{
    types::Float32Type, Array, FixedSizeListArray, RecordBatch, RecordBatchIterator, StringArray,
    UInt32Array,
};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use aws_sdk_s3::{primitives::ByteStream, Client as S3Client};
use engrave_contracts::{AreaId, MemoryId, OperationId, OperationState, TenantId};
use engrave_core::{
    retry_directive, ActorRole, ApplicationError, AuthorizationContext, DenseHit,
    DeterministicEmbeddingProvider, DomainEvent, EmbeddingProvider, IdempotencyKey, LexicalHit,
    MemoryRecord, ObjectStore, ProjectionAdapter, ProjectionIdentity, ProviderError, Repository,
    RetryDirective, RetryPolicy, SearchRequest, VersionToken, Visibility,
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
        let batches = RecordBatchIterator::new(vec![Ok(batch)].into_iter(), schema);
        self.db
            .create_table(&self.table_name, Box::new(batches))
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
