//! Deterministic Phase C pipeline semantics.
//!
//! This module is deliberately framework- and storage-free.  The SQLx worker
//! adapter persists the same invariants; keeping the transition logic here
//! makes retry, lease recovery, and resume contract-testable without a live
//! database.

use smash_contracts::{ArtifactId, ChunkId, OperationId, OperationState, SourceId, SourceState};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operation {
    pub id: OperationId,
    pub state: OperationState,
    pub attempt: u32,
    pub max_attempts: u32,
    pub lease_token: Option<String>,
    pub lease_expires_at: Option<i64>,
    pub progress: u8,
    pub checkpoint: Option<String>,
    pub cancel_requested: bool,
    pub idempotency_key: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    pub id: SourceId,
    pub state: SourceState,
    pub quarantine_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Artifact {
    pub id: ArtifactId,
    pub source_id: SourceId,
    pub processor: String,
    pub processor_version: String,
    pub input_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chunk {
    pub id: ChunkId,
    pub source_id: SourceId,
    pub representation: String,
    /// Exact source coordinate, e.g. `page=3;char=120..248`.
    pub coordinate: String,
    pub content_hash: String,
    pub artifact_id: ArtifactId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PipelineError {
    InvalidTransition { from: SourceState, to: SourceState },
    LeaseLost,
    Cancelled,
    RetryExhausted,
    Quarantined,
    IdempotencyConflict,
    InvalidCoordinate,
}

#[derive(Default)]
pub struct Pipeline {
    operations: BTreeMap<OperationId, Operation>,
    idempotency: BTreeMap<String, OperationId>,
    sources: BTreeMap<SourceId, Source>,
    artifacts: BTreeMap<(SourceId, String, String, String), ArtifactId>,
    artifact_values: BTreeMap<ArtifactId, Artifact>,
    chunks: BTreeMap<(SourceId, String, String, String), ChunkId>,
    chunk_values: BTreeMap<ChunkId, Chunk>,
    memory_activations: BTreeSet<SourceId>,
}

impl Pipeline {
    pub fn create_operation(
        &mut self,
        id: OperationId,
        idempotency_key: impl Into<String>,
        max_attempts: u32,
    ) -> Result<OperationId, PipelineError> {
        let key = idempotency_key.into();
        if let Some(existing) = self.idempotency.get(&key) {
            return if *existing == id {
                Ok(id)
            } else {
                Err(PipelineError::IdempotencyConflict)
            };
        }
        self.idempotency.insert(key.clone(), id);
        self.operations.insert(
            id,
            Operation {
                id,
                state: OperationState::Queued,
                attempt: 0,
                max_attempts: max_attempts.max(1),
                lease_token: None,
                lease_expires_at: None,
                progress: 0,
                checkpoint: None,
                cancel_requested: false,
                idempotency_key: key,
                error: None,
            },
        );
        Ok(id)
    }

    pub fn operation(&self, id: OperationId) -> Option<&Operation> {
        self.operations.get(&id)
    }

    pub fn claim(&mut self, now: i64, lease_seconds: i64) -> Option<(OperationId, String)> {
        let id = self.operations.iter().find_map(|(id, op)| {
            let available = matches!(
                op.state,
                OperationState::Queued | OperationState::Leased | OperationState::Running
            ) && op
                .lease_expires_at
                .map(|until| until <= now)
                .unwrap_or(true)
                && !op.cancel_requested;
            available.then_some(*id)
        })?;
        let op = self.operations.get_mut(&id)?;
        op.attempt += 1;
        let token = format!("{}:{}", id.as_uuid(), op.attempt);
        op.lease_token = Some(token.clone());
        op.lease_expires_at = Some(now + lease_seconds.max(1));
        op.state = OperationState::Running;
        Some((id, token))
    }

    fn leased_mut(
        &mut self,
        id: OperationId,
        token: &str,
    ) -> Result<&mut Operation, PipelineError> {
        let op = self
            .operations
            .get_mut(&id)
            .ok_or(PipelineError::LeaseLost)?;
        if op.lease_token.as_deref() != Some(token) {
            return Err(PipelineError::LeaseLost);
        }
        Ok(op)
    }

    pub fn renew(
        &mut self,
        id: OperationId,
        token: &str,
        now: i64,
        lease_seconds: i64,
    ) -> Result<(), PipelineError> {
        self.leased_mut(id, token)?.lease_expires_at = Some(now + lease_seconds.max(1));
        Ok(())
    }

    pub fn checkpoint(
        &mut self,
        id: OperationId,
        token: &str,
        progress: u8,
        value: impl Into<String>,
    ) -> Result<(), PipelineError> {
        let op = self.leased_mut(id, token)?;
        op.progress = progress.min(100);
        op.checkpoint = Some(value.into());
        Ok(())
    }

    pub fn request_cancel(&mut self, id: OperationId) -> Result<(), PipelineError> {
        let op = self
            .operations
            .get_mut(&id)
            .ok_or(PipelineError::LeaseLost)?;
        op.cancel_requested = true;
        if matches!(op.state, OperationState::Queued) {
            op.state = OperationState::Cancelled;
        }
        Ok(())
    }

    pub fn complete(&mut self, id: OperationId, token: &str) -> Result<(), PipelineError> {
        let op = self.leased_mut(id, token)?;
        if op.cancel_requested {
            op.state = OperationState::Cancelled;
            return Err(PipelineError::Cancelled);
        }
        op.state = OperationState::Succeeded;
        op.progress = 100;
        op.lease_token = None;
        op.lease_expires_at = None;
        Ok(())
    }

    pub fn fail(
        &mut self,
        id: OperationId,
        token: &str,
        error: impl Into<String>,
    ) -> Result<(), PipelineError> {
        let op = self.leased_mut(id, token)?;
        op.error = Some(error.into());
        op.lease_token = None;
        op.lease_expires_at = None;
        op.state = if op.attempt < op.max_attempts {
            OperationState::Queued
        } else {
            OperationState::Failed
        };
        Ok(())
    }

    pub fn register_source(&mut self, id: SourceId) {
        self.sources.insert(
            id,
            Source {
                id,
                state: SourceState::Uploaded,
                quarantine_reason: None,
            },
        );
    }

    pub fn transition_source(
        &mut self,
        id: SourceId,
        to: SourceState,
    ) -> Result<(), PipelineError> {
        let source = self
            .sources
            .get_mut(&id)
            .ok_or(PipelineError::Quarantined)?;
        let valid = matches!(
            (source.state, to),
            (SourceState::Uploaded, SourceState::Verified)
                | (SourceState::Verified, SourceState::Queued)
                | (SourceState::Queued, SourceState::Extracting)
                | (SourceState::Extracting, SourceState::Chunking)
                | (SourceState::Chunking, SourceState::Indexing)
                | (SourceState::Indexing, SourceState::Proposing)
                | (SourceState::Proposing, SourceState::Ready)
                | (SourceState::Indexing, SourceState::PartiallyReady)
                | (_, SourceState::Failed)
                | (_, SourceState::Quarantined)
                | (_, SourceState::Deleted)
        );
        if !valid {
            return Err(PipelineError::InvalidTransition {
                from: source.state,
                to,
            });
        }
        source.state = to;
        Ok(())
    }

    pub fn quarantine(
        &mut self,
        id: SourceId,
        reason: impl Into<String>,
    ) -> Result<(), PipelineError> {
        self.transition_source(id, SourceState::Quarantined)?;
        self.sources.get_mut(&id).unwrap().quarantine_reason = Some(reason.into());
        Ok(())
    }

    pub fn add_artifact(&mut self, artifact: Artifact) -> ArtifactId {
        let key = (
            artifact.source_id,
            artifact.processor.clone(),
            artifact.processor_version.clone(),
            artifact.input_hash.clone(),
        );
        if let Some(id) = self.artifacts.get(&key) {
            return *id;
        }
        let id = artifact.id;
        self.artifacts.insert(key, id);
        self.artifact_values.insert(id, artifact);
        id
    }

    pub fn add_chunk(&mut self, chunk: Chunk) -> Result<ChunkId, PipelineError> {
        if chunk.coordinate.trim().is_empty() || !chunk.coordinate.contains('=') {
            return Err(PipelineError::InvalidCoordinate);
        }
        let key = (
            chunk.source_id,
            chunk.representation.clone(),
            chunk.coordinate.clone(),
            chunk.content_hash.clone(),
        );
        if let Some(id) = self.chunks.get(&key) {
            return Ok(*id);
        }
        let id = chunk.id;
        self.chunks.insert(key, id);
        self.chunk_values.insert(id, chunk);
        Ok(id)
    }

    pub fn artifact_count(&self) -> usize {
        self.artifact_values.len()
    }
    pub fn chunk_count(&self) -> usize {
        self.chunk_values.len()
    }
    pub fn activate_memory_from_processor(&mut self, source_id: SourceId) {
        self.memory_activations.insert(source_id);
    }
    pub fn memory_activation_count(&self) -> usize {
        self.memory_activations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn ids() -> (OperationId, SourceId, ArtifactId, ChunkId) {
        (
            OperationId::new(Uuid::from_u128(1)),
            SourceId::new(Uuid::from_u128(2)),
            ArtifactId::new(Uuid::from_u128(3)),
            ChunkId::new(Uuid::from_u128(4)),
        )
    }

    #[test]
    fn retry_is_idempotent_and_lease_expiry_reclaims_work() {
        let (op, source, artifact, chunk) = ids();
        let mut pipeline = Pipeline::default();
        pipeline.create_operation(op, "upload:2", 3).unwrap();
        assert_eq!(pipeline.create_operation(op, "upload:2", 3).unwrap(), op);
        assert_eq!(pipeline.claim(10, 5).unwrap().0, op);
        let stale = pipeline.operation(op).unwrap().lease_token.clone().unwrap();
        pipeline
            .fail(op, &stale, "temporary parser timeout")
            .unwrap();
        let (claimed, token) = pipeline.claim(20, 5).unwrap();
        assert_eq!(claimed, op);
        pipeline.checkpoint(op, &token, 50, "page=3").unwrap();
        pipeline.renew(op, &token, 23, 5).unwrap();
        assert_eq!(pipeline.claim(27, 5), None);
        let (_, recovered_token) = pipeline.claim(29, 5).unwrap();
        assert!(pipeline.complete(op, &token).is_err());
        pipeline.complete(op, &recovered_token).unwrap();

        pipeline.register_source(source);
        for state in [
            SourceState::Verified,
            SourceState::Queued,
            SourceState::Extracting,
            SourceState::Chunking,
            SourceState::Indexing,
            SourceState::Proposing,
            SourceState::Ready,
        ] {
            pipeline.transition_source(source, state).unwrap();
        }
        let a = Artifact {
            id: artifact,
            source_id: source,
            processor: "markdown".into(),
            processor_version: "1".into(),
            input_hash: "bytes-hash".into(),
        };
        assert_eq!(pipeline.add_artifact(a.clone()), artifact);
        assert_eq!(pipeline.add_artifact(a), artifact);
        let c = Chunk {
            id: chunk,
            source_id: source,
            representation: "text".into(),
            coordinate: "page=3;char=0..10".into(),
            content_hash: "chunk-hash".into(),
            artifact_id: artifact,
        };
        assert_eq!(pipeline.add_chunk(c.clone()).unwrap(), chunk);
        assert_eq!(pipeline.add_chunk(c).unwrap(), chunk);
        assert_eq!((pipeline.artifact_count(), pipeline.chunk_count()), (1, 1));
        assert_eq!(pipeline.memory_activation_count(), 0);
    }

    #[test]
    fn cancellation_quarantine_and_checkpoint_resume_are_explicit() {
        let (op, source, _, _) = ids();
        let mut pipeline = Pipeline::default();
        pipeline.create_operation(op, "cancel:1", 1).unwrap();
        pipeline.register_source(source);
        pipeline
            .quarantine(source, "archive expansion limit exceeded")
            .unwrap();
        assert_eq!(
            pipeline.sources.get(&source).unwrap().state,
            SourceState::Quarantined
        );
        pipeline.request_cancel(op).unwrap();
        assert_eq!(
            pipeline.operation(op).unwrap().state,
            OperationState::Cancelled
        );
        assert_eq!(pipeline.claim(0, 30), None);
    }
}
