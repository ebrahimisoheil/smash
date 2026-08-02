# ADR-0019: Object-store initialization and least privilege

Status: Accepted  
Date: 2026-08-02

## Decision

MinIO is used through S3 semantics. Bootstrap credentials may create the local
bucket and least-privilege runtime identity; API and worker do not run with
root credentials. Object keys are derived from stable tenant and object IDs,
never user filenames. Initialization is idempotent and completion verifies
ownership, expected key, size, media type, and checksum before canonicalizing a
SourceVersion.
