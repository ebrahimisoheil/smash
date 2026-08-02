# 01 — The Product Thesis

> Source: SMASH_V2.md §2

## The problem

Agents do not primarily suffer from a lack of access to information. They suffer from a lack of **governed continuity**.

Documents, messages, tickets, recordings, CRM records, and databases contain observations. Vector search can find semantically related chunks. Knowledge graphs can connect entities. MCP can expose tools and resources.

None of these mechanisms alone decides:

- what an agent should remember;
- why it should remember it;
- when it applies;
- whether it is still valid;
- who approved it;
- which evidence supports it;
- which actions it must never perform.

SMASH is the layer that makes those decisions explicit.

## The category

**Agent memory control plane.**

SMASH sits between source systems and agent runtimes:

1. It accepts evidence from local files, uploads, APIs, connectors, and MCP resources.
2. It turns evidence into reviewable Proposals.
3. Approved Proposals become durable Memory.
4. Memory is retrieved through a bounded, explainable interface.
5. Rules govern what an agent may retrieve, disclose, change, or do.
6. The same Memory serves different agents and models.

## The positioning sentence

> Notion stores what a team writes. Jira tracks what a team does. SMASH governs what its agents remember.

## What SMASH deliberately is not

SMASH does not compete by becoming:

- a general document editor;
- a project tracker;
- a CRM;
- a full agent runtime.

Those categories are already mature. SMASH connects to them, preserves them as Sources, and provides the governed memory layer they do not independently provide **across agent vendors**.
