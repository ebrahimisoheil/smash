# 07 — Source Ingestion

> Source: the historical roadmap source §8

## 7.1 Supported source classes

V2 designs **one ingestion contract** that supports different adapters:

- direct file upload;
- local folder import for self-hosted installations;
- URL capture and snapshots;
- PDF, office documents, plain text, and Markdown;
- spreadsheets and structured tables;
- images, OCR, and visual description;
- audio/video and timestamped transcripts;
- email, chat, and meeting exports;
- connector objects such as Notion pages, Jira items, Drive files, and CRM records;
- MCP resources returned by approved servers.

**Do not promise every format in the first public release.** The contract must support them; the Community Edition acceptance list can remain narrow and reliable.

## 7.2 Ingestion state machine

Each Source version moves through explicit processing states:

`uploaded` → `verified` → `queued` → `extracting` → `chunking` → `indexing` → `proposing` → `ready`

Terminal or exceptional states: `partially ready`, `failed`, `quarantined`, `deleted`.

A Source can be searchable before every optional proposal step finishes, but **the UI must expose that state honestly**.

Every processor records: name, version, configuration fingerprint, input hash, output artifact IDs, warnings, execution event.

Reprocessing with a new extractor creates **new derived artifacts and projections while preserving the original bytes**.

Pipeline diagram: [23 — Diagrams §24.5](23-diagrams.md#245-source-ingestion-pipeline).

## 7.3 Safety boundary

**Source contents are untrusted data.** Text inside a PDF, webpage, or MCP resource may contain instructions aimed at the agent or processor.

- Extraction must never execute Source-provided commands.
- Proposal prompts clearly delimit Source content and treat it as **evidence, not authority**.

File validation checks:

- declared and detected media type;
- size limits;
- archive expansion limits;
- malicious path names;
- decompression bombs;
- parser failures.

Unknown or suspicious inputs are **quarantined**, not silently accepted as empty documents.

Rust's memory safety removes a large class of parser vulnerabilities but not resource exhaustion, algorithmic blowup, or logic flaws. Extraction still needs explicit size, time, recursion, and expansion limits. Prefer safe, maintained crates over hand-rolled decoding; extraction that shells out to non-Rust tooling runs as a supervised subprocess with timeouts and resource limits — see [06 §6.2](06-service-architecture.md#62-worker-uses-the-backends-core-crate).

## 7.4 Proposal generation

Extraction may produce candidate entities, relationships, claims, decisions, procedures, and Map changes. These are **Proposals**. The proposal record retains the exact evidence spans and the transformation that produced it.

An LLM may assist proposal generation, but its output is **not durable truth**. The model and prompt version become provenance.

Deterministic validation checks the following **before** the proposal reaches Review:

- required fields;
- evidence existence;
- Area scope;
- Map compatibility;
- duplicate candidates;
- contradiction candidates;
- Rule outcomes.
