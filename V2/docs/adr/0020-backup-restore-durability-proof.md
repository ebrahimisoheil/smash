# ADR-0020: Backup and restore as one durability proof

Status: Accepted  
Date: 2026-08-02

## Decision

PostgreSQL and MinIO are backed up and restored together. Replacing containers
is not a backup test; a clean named-volume environment must recover canonical
metadata, Events, idempotency records, Decision Envelope snapshots, and the
original Sales fixture bytes. Reset is explicitly destructive and is never
described as restore.

The local proof uses documented tooling and placeholder key references. It does
not claim to be a production retention, encryption, or managed-service policy.
