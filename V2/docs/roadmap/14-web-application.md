# 14 — Web Application Requirements

> Source: SMASH_V2.md §15

## 14.1 Home

Home answers three questions: **what should my agents know now, what changed, and what needs my attention?**

It shows:

- a small set of active Memory with reasons;
- review queue counts;
- recent agent Activity;
- connected agent health;
- quick Source capture.

## 14.2 Areas

An Area provides **Board**, **Graph**, and **Rules** views.

**Board** organizes current objects by Map kinds. Searchable, and columnar where that matches the domain — especially Sales. Cards open a universal saved-record view.

**Graph** uses compact nodes and relationships with selective labels. Hover previews content on pointer devices; click or tap opens the full record. Search focuses the network. **Large graphs load bounded neighborhoods, never every record by default.**

**Rules** shows active policies, effects, rationale, source authority, trigger history, and tests.

## 14.3 Library

Library lists Sources in every supported format. It exposes:

- origin;
- Area;
- permissions;
- processing state;
- versions;
- extraction artifacts;
- proposed/accepted Memory;
- failures.

Users can upload, reconnect, retry, quarantine, archive, and inspect exact evidence.

## 14.4 Review

Review is **one inbox** for Memory Proposals, Map changes, Cross-Map mappings, conflicts, and Rule proposals.

The interface prioritizes reason, exact evidence, scope, applicability, duplicate candidates, and consequences.

**Acceptance should be quick, but must never hide what changes.**

## 14.5 Saved record

Every saved object uses a consistent detail surface showing: type, title, description or claim, reason, evidence, connections, status, version, lineage, Activity, and actions.

- Desktop: drawer or page.
- Mobile: bottom sheet or full screen.

## 14.6 Search

Search is available globally and inside Areas.

- **Light search is the default.**
- **Aggressive search is an explicit option** or an escalation surfaced when useful.
- Users see which Areas and Sources were searched, and why results appeared.

## 14.7 Mobile

Mobile prioritizes **capture, search, review, and reading saved records**.

Share-sheet uploads come eventually. First, the responsive web application must make these low-friction:

- add Source;
- accept / edit / reject Proposal;
- ask / search;
- view recent Areas.

Complex Map editing and large graph exploration are **secondary** mobile capabilities.

## Accessibility

Accessibility is a release requirement, not a polish item: semantic structure, focus management, contrast, labels, reduced motion. See [17 — Testing §17.6](17-testing-evaluation.md#176-ui-tests).
