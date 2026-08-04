---
name: engrave-sources
version: 1.0.0
description: Queue Source ingestion with attributable evidence.
---

# ENGRAVE Sources

Use `ingest_source` for an explicit Source and stable external ID. Interactive
calls queue work; extraction, chunking, quarantine, and indexing belong to the
worker. Report operation identity and do not disclose quarantined content.
